use std::fs;
use std::path::Path;

use anyhow::{Result, bail};

use crate::ai::context::build_actor_dialogue_context;
use crate::app::projection::{
    entity_summary as build_entity_summary, place_summary as build_place_summary,
};
use crate::app::query::manual_actor_id;
use crate::app::read_model::build_ui_snapshot;
use crate::domain::commands::{ActionKind, ActionRequest};
use crate::domain::events::{
    ActionResult, ContextEntry, DialogueLine, DialogueSpeaker, EntitySummary, GameEvent,
    PlaceSummary, SystemContext,
};
use crate::domain::seed::WorldSeed;
use crate::domain::time::{GameTime, TimeDelta};
use crate::llm::LlmBackend;
use crate::simulation::GameState;
use crate::world::{ActorId, ControllerMode, EntityId, PlaceId, PlaceKind, World};

const START_TIME: GameTime = GameTime::from_seconds(8 * 60 * 60);
const INSPECT_TIME: TimeDelta = TimeDelta::from_seconds(10);

#[derive(Debug)]
pub struct GameService<B> {
    state: GameState,
    backend: B,
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

        Ok(Self {
            state: GameState { world },
            backend,
        })
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    pub fn snapshot(&self) -> crate::simulation::UiSnapshot {
        build_ui_snapshot(&self.state)
    }

    pub async fn apply_action(&mut self, request: ActionRequest) -> Result<ActionResult> {
        if !self.state.world.actor_ids().contains(&request.actor_id) {
            bail!("Actor does not exist.");
        }

        let events = match request.action {
            ActionKind::MoveTo { destination } => self.move_to(request.actor_id, destination)?,
            ActionKind::Speak { target, text } => self.speak(request.actor_id, target, text).await?,
            ActionKind::InspectEntity { entity_id } => {
                vec![self.inspect_entity(request.actor_id, entity_id)?]
            }
            ActionKind::Wait { duration } => {
                vec![self.wait_for(request.actor_id, duration.max(TimeDelta::ONE_SECOND))]
            }
        };
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
        Ok(())
    }

    async fn speak(
        &mut self,
        actor_id: ActorId,
        target_id: ActorId,
        text: String,
    ) -> Result<Vec<GameEvent>> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        if actor_id == target_id {
            bail!("You cannot talk to yourself.");
        }

        let actor_place_id = self
            .state
            .world
            .actor_place_id(actor_id)
            .ok_or_else(|| anyhow::anyhow!("Actor is not in a place."))?;
        if self.state.world.actor_place_id(target_id) != Some(actor_place_id) {
            bail!("That person is no longer here.");
        }

        let city_id = self
            .state
            .world
            .place_city_id(actor_place_id)
            .expect("actor place should belong to a city");
        let started_at = self.current_time();
        let actor_line = DialogueLine {
            timestamp: started_at,
            speaker: DialogueSpeaker::Actor(actor_id),
            text: trimmed.to_string(),
        };
        let mut transcript = vec![actor_line.clone()];
        let mut events = vec![GameEvent::SpeechLineRecorded { line: actor_line }];
        let actor_duration = line_duration(trimmed);
        let mut total_duration = actor_duration;

        if self.state.world.actor(target_id).controller == ControllerMode::AiAgent {
            let memory = self
                .state
                .world
                .actor_conversation_memory(target_id)
                .unwrap_or_default();
            let context = build_actor_dialogue_context(
                &self.state.world,
                started_at,
                city_id,
                target_id,
                actor_id,
                &memory,
                trimmed.to_string(),
            )?;

            let response = self.backend.generate_dialogue(&context).await?;
            let response_duration = line_duration(&response.text);
            let response_line = DialogueLine {
                timestamp: started_at.advance(actor_duration),
                speaker: DialogueSpeaker::Actor(target_id),
                text: response.text,
            };
            total_duration = total_duration.saturating_add(response_duration);
            transcript.push(response_line.clone());
            events.push(GameEvent::SpeechLineRecorded {
                line: response_line,
            });
        }

        self.state.world.record_speech_process(
            actor_id,
            target_id,
            actor_place_id,
            started_at,
            total_duration,
            transcript.clone(),
        );
        self.advance_time(total_duration);

        let memory_summary = self.backend.summarize_memory(&transcript).await?;
        self.state
            .world
            .merge_actor_conversation_memory(actor_id, memory_summary.clone());
        self.state
            .world
            .merge_actor_conversation_memory(target_id, memory_summary);

        Ok(events)
    }

    fn move_to(&mut self, actor_id: ActorId, destination_id: PlaceId) -> Result<Vec<GameEvent>> {
        let current_place_id = self
            .state
            .world
            .actor_place_id(actor_id)
            .ok_or_else(|| anyhow::anyhow!("Actor is not in a place."))?;
        let (resolved_destination_id, route) = self
            .state
            .world
            .place_routes(current_place_id)
            .into_iter()
            .find(|(place_id, _)| *place_id == destination_id)
            .ok_or_else(|| anyhow::anyhow!("Selected route is no longer available."))?;
        let travel_time = route.travel_time;
        let started_at = self.current_time();

        self.state
            .world
            .record_travel_process(actor_id, resolved_destination_id, started_at, travel_time);
        self.state.world.move_actor(actor_id, resolved_destination_id);
        self.advance_time(travel_time);
        self.learn_city(
            actor_id,
            self.state
                .world
                .place_city_id(resolved_destination_id)
                .expect("destination should belong to a city"),
        );

        let destination = self.place_summary(resolved_destination_id);
        let context_event = self.push_system_context(
            actor_id,
            self.current_time(),
            SystemContext::Travel {
                destination,
                duration: travel_time,
            },
        );
        Ok(vec![
            context_event,
            GameEvent::TravelCompleted {
                destination,
                route,
                duration: travel_time,
            },
        ])
    }

    fn inspect_entity(&mut self, actor_id: ActorId, entity_id: EntityId) -> Result<GameEvent> {
        let place_id = self
            .state
            .world
            .actor_place_id(actor_id)
            .ok_or_else(|| anyhow::anyhow!("Actor is not in a place."))?;
        let is_here = self.state.world.place_entities(place_id).contains(&entity_id);
        if !is_here {
            bail!("That entity is no longer here.");
        }
        let started_at = self.current_time();
        self.state.world.record_inspect_process(
            actor_id,
            entity_id,
            place_id,
            started_at,
            INSPECT_TIME,
        );
        self.advance_time(INSPECT_TIME);
        Ok(GameEvent::EntityInspected {
            entity: self.entity_summary(entity_id),
        })
    }

    fn wait_for(&mut self, actor_id: ActorId, duration: TimeDelta) -> GameEvent {
        let duration = duration.max(TimeDelta::ONE_SECOND);
        let place_id = self
            .state
            .world
            .actor_place_id(actor_id)
            .expect("actor should occupy a place");
        let started_at = self.current_time();
        self.state
            .world
            .record_waiting_process(actor_id, place_id, started_at, duration);
        self.advance_time(duration);
        GameEvent::WaitCompleted {
            duration,
            current_time: self.current_time(),
        }
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

    pub fn manual_actor_id(&self) -> ActorId {
        manual_actor_id(&self.state)
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

    use crate::domain::commands::{ActionKind, ActionRequest};
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
    async fn speak_action_uses_typed_action_path() {
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

        assert!(
            result
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::SpeechLineRecorded { .. }))
        );
        assert!(
            game.snapshot()
                .context_feed
                .iter()
                .any(|entry| matches!(entry, crate::domain::events::ContextEntry::Dialogue(_)))
        );
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
