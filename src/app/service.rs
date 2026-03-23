use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

use anyhow::{Result, bail};
use tracing::{debug, error, info, trace, warn};

use crate::ai::context::build_actor_turn_context;
use crate::app::projection::{
    entity_summary as build_entity_summary, place_summary as build_place_summary,
};
use crate::app::read_model::build_ui_snapshot;
use crate::domain::commands::{
    ActionKind, ActionPlan, ActionRequest, AgentAvailableAction, AvailableAction, PlannedAction,
};
use crate::domain::events::{
    ActionResult, ContextEntry, DialogueLine, DialogueSpeaker, EntitySummary, GameEvent, PlaceSummary,
    SystemContext,
};
use crate::domain::seed::WorldSeed;
use crate::domain::time::{GameTime, TimeDelta};
use crate::llm::LlmBackend;
use crate::simulation::{AgentDebugSnapshot, GameState};
use crate::world::{ActorId, ControllerMode, EntityId, PlaceId, PlaceKind, World};
use crate::llm::AgentDebugTrace;
use crate::app::projection::actor_view as build_actor_view;

const START_TIME: GameTime = GameTime::from_seconds(8 * 60 * 60);
const INSPECT_TIME: TimeDelta = TimeDelta::from_seconds(10);
#[derive(Debug)]
pub struct GameService<B> {
    state: GameState,
    backend: B,
    agent_debug_traces: BTreeMap<ActorId, AgentDebugTrace>,
}

impl<B: LlmBackend> GameService<B> {
    pub fn new(backend: B) -> Result<Self> {
        let seed = WorldSeed::new(42);
        let mut world = World::generate(seed, 18);
        let actor_id = world
            .manual_actor_id()
            .expect("generated world should contain a manual actor");
        let start_city_id = world.actor_city_id(actor_id).unwrap_or_else(|| world.city_ids()[0]);
        let city_places = world.city_places(start_city_id);
        let start_place_id = city_places
            .iter()
            .copied()
            .find(|place_id| matches!(world.place(*place_id).kind, PlaceKind::Residence))
            .or_else(|| {
                city_places
                    .iter()
                    .copied()
                    .find(|place_id| world.place(*place_id).kind.supports_people())
            })
            .or_else(|| city_places.first().copied())
            .expect("generated city should have places");
        world.set_actor_home(actor_id, start_place_id);
        world.move_actor(actor_id, start_place_id);
        world.set_current_time(START_TIME);
        world.discover_city(actor_id, start_city_id, START_TIME);
        for city_id in world.city_connections(start_city_id) {
            world.discover_city(actor_id, city_id, START_TIME);
        }
        world.append_context_entry(
            actor_id,
            ContextEntry::System {
                timestamp: START_TIME,
                context: SystemContext::Start,
            },
        );
        validate_world(&world)?;
        info!(
            backend = %backend.label(),
            world_seed = seed.raw(),
            manual_actor_id = actor_id.index(),
            start_city_id = start_city_id.index(),
            start_place_id = start_place_id.index(),
            "initialized game service"
        );

        Ok(Self {
            state: GameState { world },
            backend,
            agent_debug_traces: BTreeMap::new(),
        })
    }

    pub fn backend_label(&self) -> String {
        self.backend.label()
    }

    pub fn snapshot(&self) -> crate::simulation::UiSnapshot {
        self.snapshot_for(self.manual_actor_id())
    }

    pub fn snapshot_for(&self, focused_actor_id: ActorId) -> crate::simulation::UiSnapshot {
        build_ui_snapshot(
            &self.state,
            focused_actor_id,
            self.available_actions(focused_actor_id),
            self.local_agent_debug_snapshots(focused_actor_id),
        )
    }

    pub fn available_actions(&self, actor_id: ActorId) -> Vec<AvailableAction> {
        let Some(place_id) = self.state.world.actor_place_id(actor_id) else {
            return Vec::new();
        };

        let mut actions = self
            .state
            .world
            .place_routes(place_id)
            .into_iter()
            .map(|(destination, _)| AvailableAction::MoveTo { destination })
            .collect::<Vec<_>>();
        actions.extend(
            self.state
                .world
                .place_actors(place_id)
                .into_iter()
                .filter(|candidate| *candidate != actor_id)
                .map(|target| AvailableAction::SpeakTo { target }),
        );
        actions.extend(
            self.state
                .world
                .place_entities(place_id)
                .into_iter()
                .map(|entity_id| AvailableAction::InspectEntity { entity_id }),
        );
        actions.push(AvailableAction::Wait);
        actions
    }

    pub fn available_agent_actions(&self, actor_id: ActorId) -> Vec<AgentAvailableAction> {
        let Some(place_id) = self.state.world.actor_place_id(actor_id) else {
            return vec![AgentAvailableAction::DoNothing];
        };

        let mut actions = self
            .state
            .world
            .place_routes(place_id)
            .into_iter()
            .map(|(destination, _)| AgentAvailableAction::MoveTo { destination })
            .collect::<Vec<_>>();
        actions.extend(
            self.state
                .world
                .place_actors(place_id)
                .into_iter()
                .filter(|candidate| *candidate != actor_id)
                .map(|target| AgentAvailableAction::SpeakTo { target }),
        );
        actions.extend(
            self.state
                .world
                .place_entities(place_id)
                .into_iter()
                .map(|entity_id| AgentAvailableAction::InspectEntity { entity_id }),
        );
        actions.push(AgentAvailableAction::DoNothing);
        actions
    }

    pub fn plan_action(&self, request: ActionRequest) -> Result<ActionPlan> {
        debug!(
            actor_id = request.actor_id.index(),
            action = ?request.action,
            "planning action"
        );
        if !self.state.world.actor_ids().contains(&request.actor_id) {
            bail!("Actor does not exist.");
        }

        let actor_place_id = self
            .state
            .world
            .actor_place_id(request.actor_id)
            .ok_or_else(|| anyhow::anyhow!("Actor is not in a place."))?;
        let available_actions = self.available_actions(request.actor_id);

        match request.action {
            ActionKind::MoveTo { destination } => {
                let route = self
                    .state
                    .world
                    .place_routes(actor_place_id)
                    .into_iter()
                    .find_map(|(candidate, route)| (candidate == destination).then_some(route))
                    .ok_or_else(|| anyhow::anyhow!("Selected route is no longer available."))?;
                Ok(ActionPlan {
                    actor_id: request.actor_id,
                    duration: route.travel_time,
                    action: PlannedAction::MoveTo {
                        origin: actor_place_id,
                        destination,
                        route,
                    },
                })
            }
            ActionKind::Speak { target, text } => {
                if !available_actions.contains(&AvailableAction::SpeakTo { target }) {
                    bail!("That person is no longer here.");
                }
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    bail!("You cannot say nothing.");
                }
                Ok(ActionPlan {
                    actor_id: request.actor_id,
                    duration: line_duration(trimmed),
                    action: PlannedAction::Speak {
                        place_id: actor_place_id,
                        target,
                        text: trimmed.to_string(),
                    },
                })
            }
            ActionKind::InspectEntity { entity_id } => {
                if !available_actions.contains(&AvailableAction::InspectEntity { entity_id }) {
                    bail!("That entity is no longer here.");
                }
                Ok(ActionPlan {
                    actor_id: request.actor_id,
                    duration: INSPECT_TIME,
                    action: PlannedAction::InspectEntity {
                        place_id: actor_place_id,
                        entity_id,
                    },
                })
            }
            ActionKind::Wait { duration } => {
                if !available_actions.contains(&AvailableAction::Wait) {
                    bail!("This actor cannot wait right now.");
                }
                Ok(ActionPlan {
                    actor_id: request.actor_id,
                    duration: duration.max(TimeDelta::ONE_SECOND),
                    action: PlannedAction::Wait {
                        place_id: actor_place_id,
                    },
                })
            }
            ActionKind::DoNothing => Ok(ActionPlan {
                actor_id: request.actor_id,
                duration: TimeDelta::ZERO,
                action: PlannedAction::DoNothing {
                    place_id: actor_place_id,
                },
            }),
        }
    }

    pub async fn choose_autonomous_action(&mut self, actor_id: ActorId) -> Result<Option<ActionRequest>> {
        debug!(actor_id = actor_id.index(), "choosing autonomous action");
        if !self.state.world.actor_ids().contains(&actor_id) {
            bail!("Actor does not exist.");
        }
        if self.state.world.actor(actor_id).controller != ControllerMode::AiAgent {
            return Ok(None);
        }

        let available_actions = self.available_agent_actions(actor_id);
        let context = build_actor_turn_context(
            &self.state.world,
            self.current_time(),
            actor_id,
            available_actions,
        )?;
        trace!(
            actor_id = actor_id.index(),
            available_actions = ?context.available_actions,
            recent_speech = ?context.recent_speech,
            "built autonomous actor turn context"
        );
        let selection_started = Instant::now();
        let selection = match self.backend.choose_action(&context).await {
            Ok(selection) => selection,
            Err(error) => {
                let mut trace = AgentDebugTrace::from_context(&context, &self.backend.label());
                trace.error = Some(error.to_string());
                self.agent_debug_traces.insert(actor_id, trace);
                error!(
                    actor_id = actor_id.index(),
                    error = %error,
                    elapsed_ms = selection_started.elapsed().as_millis(),
                    "autonomous action selection failed"
                );
                return Ok(None);
            }
        };
        self.agent_debug_traces
            .insert(actor_id, selection.trace.clone());
        info!(
            actor_id = actor_id.index(),
            selected_action = ?selection.action,
            tool_calls = selection.trace.tool_calls.len(),
            elapsed_ms = selection_started.elapsed().as_millis(),
            "autonomous actor selected action"
        );

        Ok(Some(ActionRequest {
            actor_id,
            action: selection.action,
        }))
    }

    pub async fn apply_action(&mut self, request: ActionRequest) -> Result<ActionResult> {
        info!(
            actor_id = request.actor_id.index(),
            action = ?request.action,
            "applying action"
        );
        let actor_id = request.actor_id;
        let plan = self.plan_action(request)?;
        debug!(
            actor_id = actor_id.index(),
            duration_seconds = plan.duration.seconds(),
            planned_action = ?plan.action,
            "planned action"
        );
        let mut events = self.execute_plan(plan).await?;
        if let Some(place_id) = self.state.world.actor_place_id(actor_id) {
            events.extend(self.run_autonomous_turns(place_id).await?);
        }
        info!(
            actor_id = actor_id.index(),
            event_count = events.len(),
            "action application complete"
        );
        Ok(ActionResult {
            events,
            should_quit: false,
        })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(&self.state)?;
        fs::write(path, data)?;
        Ok(())
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        let data = fs::read_to_string(path)?;
        let state = serde_json::from_str::<GameState>(&data)?;
        validate_world(&state.world)?;
        self.state = state;
        self.agent_debug_traces.clear();
        info!(path = %path.display(), "loaded game state");
        Ok(())
    }

    async fn execute_plan(&mut self, plan: ActionPlan) -> Result<Vec<GameEvent>> {
        debug!(
            actor_id = plan.actor_id.index(),
            duration_seconds = plan.duration.seconds(),
            planned_action = ?plan.action,
            "executing action plan"
        );
        match plan.action {
            PlannedAction::MoveTo {
                destination,
                route,
                ..
            } => {
                let started_at = self.current_time();
                self.state.world.record_travel_process(
                    plan.actor_id,
                    destination,
                    started_at,
                    plan.duration,
                );
                self.state.world.move_actor(plan.actor_id, destination);
                self.advance_time(plan.duration);
                self.learn_city(
                    plan.actor_id,
                    self.state
                        .world
                        .place_city_id(destination)
                        .expect("destination should belong to a city"),
                );

                let destination = self.place_summary(destination);
                let context_event = self.push_system_context(
                    plan.actor_id,
                    self.current_time(),
                    SystemContext::Travel {
                        destination,
                        duration: plan.duration,
                    },
                );
                info!(
                    actor_id = plan.actor_id.index(),
                    destination_id = destination.id.index(),
                    duration_seconds = plan.duration.seconds(),
                    route_kind = route.kind.label(),
                    "travel completed"
                );
                Ok(vec![
                    context_event,
                    GameEvent::TravelCompleted {
                        destination,
                        route,
                        duration: plan.duration,
                    },
                ])
            }
            PlannedAction::Speak {
                place_id,
                target,
                text,
            } => {
                let started_at = self.current_time();
                let line = DialogueLine {
                    timestamp: started_at,
                    speaker: DialogueSpeaker::Actor(plan.actor_id),
                    text,
                };
                self.state.world.record_speech_process(
                    plan.actor_id,
                    target,
                    place_id,
                    started_at,
                    plan.duration,
                    vec![line.clone()],
                );
                self.advance_time(plan.duration);

                let transcript = self.state.world.speech_lines_between(plan.actor_id, target, 64);
                let memory_summary = self.backend.summarize_memory(&transcript).await?;
                self.state
                    .world
                    .merge_actor_conversation_memory(plan.actor_id, memory_summary.clone());
                self.state
                    .world
                    .merge_actor_conversation_memory(target, memory_summary);
                info!(
                    actor_id = plan.actor_id.index(),
                    target_id = target.index(),
                    text = %line.text,
                    duration_seconds = plan.duration.seconds(),
                    "speech recorded"
                );

                Ok(vec![GameEvent::SpeechLineRecorded { line }])
            }
            PlannedAction::InspectEntity { entity_id, place_id } => {
                let started_at = self.current_time();
                self.state.world.record_inspect_process(
                    plan.actor_id,
                    entity_id,
                    place_id,
                    started_at,
                    plan.duration,
                );
                self.advance_time(plan.duration);
                info!(
                    actor_id = plan.actor_id.index(),
                    entity_id = entity_id.index(),
                    duration_seconds = plan.duration.seconds(),
                    "entity inspected"
                );
                Ok(vec![GameEvent::EntityInspected {
                    entity: self.entity_summary(entity_id),
                }])
            }
            PlannedAction::Wait { place_id } => {
                let started_at = self.current_time();
                self.state.world.record_waiting_process(
                    plan.actor_id,
                    place_id,
                    started_at,
                    plan.duration,
                );
                self.advance_time(plan.duration);
                info!(
                    actor_id = plan.actor_id.index(),
                    duration_seconds = plan.duration.seconds(),
                    "wait completed"
                );
                Ok(vec![GameEvent::WaitCompleted {
                    duration: plan.duration,
                    current_time: self.current_time(),
                }])
            }
            PlannedAction::DoNothing { place_id } => {
                self.state.world.record_do_nothing_process(
                    plan.actor_id,
                    place_id,
                    self.current_time(),
                );
                info!(actor_id = plan.actor_id.index(), "actor chose do_nothing");
                Ok(Vec::new())
            }
        }
    }

    async fn run_autonomous_turns(&mut self, place_id: PlaceId) -> Result<Vec<GameEvent>> {
        let mut events = Vec::new();
        let ai_actors = self
            .state
            .world
            .place_actors(place_id)
            .into_iter()
            .filter(|actor_id| self.state.world.actor(*actor_id).controller == ControllerMode::AiAgent)
            .collect::<Vec<_>>();
        debug!(
            place_id = place_id.index(),
            ai_actor_ids = ?ai_actors.iter().map(|id| id.index()).collect::<Vec<_>>(),
            "running autonomous turns"
        );

        for actor_id in ai_actors {
            if self.state.world.actor_place_id(actor_id) != Some(place_id) {
                warn!(actor_id = actor_id.index(), place_id = place_id.index(), "skipping autonomous actor that moved before its turn");
                continue;
            }
            let Some(request) = self.choose_autonomous_action(actor_id).await? else {
                continue;
            };
            let plan = self.plan_action(request)?;
            events.extend(self.execute_plan(plan).await?);
        }

        Ok(events)
    }

    fn push_system_context(
        &mut self,
        actor_id: ActorId,
        timestamp: GameTime,
        context: SystemContext,
    ) -> GameEvent {
        let entry = ContextEntry::System { timestamp, context };
        self.state.world.append_context_entry(actor_id, entry.clone());
        GameEvent::ContextAppended { entry }
    }

    fn advance_time(&mut self, duration: TimeDelta) {
        let next_time = self.current_time().advance(duration);
        self.state.world.set_current_time(next_time);
    }

    fn learn_city(&mut self, actor_id: ActorId, city_id: crate::world::CityId) {
        self.state
            .world
            .discover_city(actor_id, city_id, self.current_time());
        for connected in self.state.world.city_connections(city_id) {
            self.state
                .world
                .discover_city(actor_id, connected, self.current_time());
        }
    }

    fn current_time(&self) -> GameTime {
        self.state.world.current_time()
    }

    fn local_agent_debug_snapshots(&self, focused_actor_id: ActorId) -> Vec<AgentDebugSnapshot> {
        let Some(place_id) = self.state.world.actor_place_id(focused_actor_id) else {
            return Vec::new();
        };

        self.state
            .world
            .place_actors(place_id)
            .into_iter()
            .filter(|actor_id| {
                *actor_id != focused_actor_id
                    && self.state.world.actor(*actor_id).controller == ControllerMode::AiAgent
            })
            .map(|actor_id| AgentDebugSnapshot {
                actor: build_actor_view(&self.state.world, actor_id),
                trace: self.agent_debug_traces.get(&actor_id).cloned(),
            })
            .collect()
    }

    pub fn manual_actor_id(&self) -> ActorId {
        self.state
            .world
            .manual_actor_id()
            .expect("world should contain a manual actor")
    }

    pub fn actor_exists(&self, actor_id: ActorId) -> bool {
        self.state.world.actor_ids().contains(&actor_id)
    }

    pub fn agent_debug_trace(&self, actor_id: ActorId) -> Option<AgentDebugTrace> {
        self.agent_debug_traces.get(&actor_id).cloned()
    }

    fn place_summary(&self, place_id: PlaceId) -> PlaceSummary {
        build_place_summary(&self.state.world, place_id)
    }

    fn entity_summary(&self, entity_id: EntityId) -> EntitySummary {
        build_entity_summary(&self.state.world, entity_id)
    }

}

fn line_duration(text: &str) -> TimeDelta {
    let word_count = text.split_whitespace().count().max(1) as u32;
    TimeDelta::from_seconds(4 + word_count.saturating_mul(2))
}

fn validate_world(world: &World) -> Result<()> {
    let violations = world.validate();
    if violations.is_empty() {
        Ok(())
    } else {
        bail!("world validation failed: {violations:?}");
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use petgraph::visit::EdgeRef;
    use serde_json::to_vec_pretty;

    use crate::domain::commands::{
        ActionKind, ActionRequest, AgentAvailableAction, AvailableAction, PlannedAction,
    };
    use crate::domain::events::GameEvent;
    use crate::domain::time::TimeDelta;
    use crate::llm::{LlmBackend, MockBackend};
    use crate::simulation::Interactable;
    use crate::world::{WorldNode, WorldRelation};

    use super::GameService;

    fn nearby_actor_id<B: LlmBackend>(game: &GameService<B>) -> crate::world::ActorId {
        game.snapshot()
            .interactables
            .into_iter()
            .find_map(|interactable| match interactable {
                Interactable::Talk(actor) => Some(actor.id),
                _ => None,
            })
            .expect("expected a nearby actor")
    }

    #[tokio::test]
    async fn speak_action_triggers_autonomous_ai_reply_turn() {
        let mut game = GameService::new(MockBackend).unwrap();
        let actor_id = game.manual_actor_id();
        let target_id = nearby_actor_id(&game);

        let result = game
            .apply_action(ActionRequest {
                actor_id,
                action: ActionKind::Speak {
                    target: target_id,
                    text: "hello".to_string(),
                },
            })
            .await
            .unwrap();

        assert_eq!(result.events.len(), 2);
        assert!(result
            .events
            .iter()
            .all(|event| matches!(event, GameEvent::SpeechLineRecorded { .. })));
        assert!(
            game.snapshot()
                .context_feed
                .iter()
                .any(|entry| matches!(entry, crate::domain::events::ContextEntry::Dialogue(_)))
        );
        assert_eq!(
            game.state.world.speech_lines_between(actor_id, target_id, 8).len(),
            2
        );
    }

    #[tokio::test]
    async fn choose_autonomous_action_can_choose_do_nothing() {
        let mut game = GameService::new(MockBackend).unwrap();
        let actor_id = game.manual_actor_id();
        let target_id = nearby_actor_id(&game);

        game.apply_action(ActionRequest {
            actor_id,
            action: ActionKind::Speak {
                target: target_id,
                text: "hello".to_string(),
            },
        })
        .await
        .unwrap();

        let follow_up = game.choose_autonomous_action(target_id).await.unwrap();
        assert!(matches!(
            follow_up,
            Some(ActionRequest {
                actor_id,
                action: ActionKind::DoNothing,
            }) if actor_id == target_id
        ));
    }

    #[test]
    fn snapshot_for_renders_any_focused_actor() {
        let game = GameService::new(MockBackend).unwrap();
        let target_id = nearby_actor_id(&game);

        let snapshot = game.snapshot_for(target_id);

        assert_eq!(snapshot.focused_actor_id, target_id);
        assert_eq!(snapshot.status.id, target_id);
        assert!(
            snapshot
                .available_actions
                .contains(&AvailableAction::Wait)
        );
    }

    #[test]
    fn available_actions_and_plan_action_share_the_same_surface() {
        let game = GameService::new(MockBackend).unwrap();
        let actor_id = game.manual_actor_id();
        let actions = game.available_actions(actor_id);

        let move_destination = actions
            .iter()
            .find_map(|action| match action {
                AvailableAction::MoveTo { destination } => Some(*destination),
                _ => None,
            })
            .expect("expected at least one move action");
        let talk_target = actions
            .iter()
            .find_map(|action| match action {
                AvailableAction::SpeakTo { target } => Some(*target),
                _ => None,
            })
            .expect("expected at least one speak action");

        let move_plan = game
            .plan_action(ActionRequest {
                actor_id,
                action: ActionKind::MoveTo {
                    destination: move_destination,
                },
            })
            .unwrap();
        let speak_plan = game
            .plan_action(ActionRequest {
                actor_id,
                action: ActionKind::Speak {
                    target: talk_target,
                    text: "hello".to_string(),
                },
            })
            .unwrap();

        assert!(move_plan.duration > TimeDelta::ZERO);
        assert!(matches!(
            move_plan.action,
            PlannedAction::MoveTo { destination, .. } if destination == move_destination
        ));
        assert!(matches!(
            speak_plan.action,
            PlannedAction::Speak { target, .. } if target == talk_target
        ));
    }

    #[test]
    fn agent_action_surface_exposes_do_nothing_but_not_wait() {
        let game = GameService::new(MockBackend).unwrap();
        let actor_id = nearby_actor_id(&game);
        let actions = game.available_agent_actions(actor_id);

        assert!(actions.contains(&AgentAvailableAction::DoNothing));
        assert!(actions
            .iter()
            .any(|action| matches!(action, AgentAvailableAction::SpeakTo { .. })));
    }

    #[tokio::test]
    async fn save_and_load_round_trip() {
        let mut game = GameService::new(MockBackend).unwrap();
        game.apply_action(ActionRequest {
            actor_id: game.manual_actor_id(),
            action: ActionKind::Wait {
                duration: TimeDelta::from_seconds(60),
            },
        })
        .await
        .unwrap();
        game.save(Path::new("/tmp/riggy-test-save.json")).unwrap();

        let mut loaded = GameService::new(MockBackend).unwrap();
        loaded.load(Path::new("/tmp/riggy-test-save.json")).unwrap();
        assert_eq!(
            game.state.world.current_time(),
            loaded.state.world.current_time()
        );
        let loaded_actor_id = loaded.state.world.manual_actor_id().unwrap();
        let game_actor_id = game.state.world.manual_actor_id().unwrap();
        assert_eq!(
            game.state.world.actor_city_id(game_actor_id),
            loaded.state.world.actor_city_id(loaded_actor_id)
        );
    }

    #[tokio::test]
    async fn load_rejects_invalid_world_snapshot() {
        let mut game = GameService::new(MockBackend).unwrap();
        let actor_id = game
            .state
            .world
            .actor_ids()
            .into_iter()
            .find(|candidate| *candidate != game.manual_actor_id())
            .unwrap();
        let resident_city_id = game.state.world.actor_resident_city_ids(actor_id)[0];
        let other_city_id = game
            .state
            .world
            .city_ids()
            .into_iter()
            .find(|city_id| *city_id != resident_city_id)
            .expect("world should have another city");
        let other_place_id = game.state.world.city_places(other_city_id)[0];
        let present_edge_id = game
            .state
            .world
            .graph
            .edges_directed(actor_id.0, petgraph::Direction::Outgoing)
            .find(|edge| {
                matches!(edge.weight(), WorldRelation::LocatedAt)
                    && matches!(
                        game.state.world.graph.node_weight(edge.target()),
                        Some(WorldNode::Place(_))
                    )
            })
            .map(|edge| edge.id())
            .expect("actor should have a present place");
        game.state.world.graph.remove_edge(present_edge_id);
        game.state.world.graph.add_edge(
            actor_id.0,
            other_place_id.0,
            WorldRelation::LocatedAt,
        );

        let invalid_path = Path::new("/tmp/riggy-invalid-save.json");
        std::fs::write(invalid_path, to_vec_pretty(&game.state).unwrap()).unwrap();

        let mut loaded = GameService::new(MockBackend).unwrap();
        let err = loaded.load(invalid_path).unwrap_err();
        assert!(err.to_string().contains("world validation failed"));
    }
}
