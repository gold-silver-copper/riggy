use std::fs;
use std::path::Path;

use anyhow::{Result, bail};

use crate::ai::context::build_npc_dialogue_context;
use crate::app::projection::{
    entity_summary as build_entity_summary, place_summary as build_place_summary,
};
use crate::app::query::{
    active_dialogue_npc_id, active_dialogue_process_id, current_city_id, current_place_id,
    current_time, player_id,
};
use crate::app::read_model::build_ui_snapshot;
use crate::domain::commands::GameCommand;
use crate::domain::events::{
    CommandResult, ContextEntry, DialogueLine, DialogueSpeaker, EntitySummary, GameEvent,
    PlaceSummary, SystemContext,
};
use crate::domain::seed::WorldSeed;
use crate::domain::time::{GameTime, TimeDelta};
use crate::llm::LlmBackend;
use crate::simulation::{GameState, UiSnapshot};
use crate::world::{EntityId, NpcId, PlaceId, PlaceKind, World};

const START_TIME: GameTime = GameTime::from_seconds(8 * 60 * 60);
const DIALOGUE_TIME: TimeDelta = TimeDelta::from_seconds(30);

#[derive(Debug)]
pub struct GameService<B> {
    state: GameState,
    backend: B,
}

impl<B: LlmBackend> GameService<B> {
    pub fn new(backend: B) -> Result<Self> {
        let seed = WorldSeed::new(42);
        let mut world = World::generate(seed, 18);
        validate_world(&world)?;
        let start_city_id = world.city_ids()[0];
        let city_places = world.city_places(start_city_id);
        let start_place_id = city_places
            .iter()
            .copied()
            .find(|place_id| matches!(world.place(*place_id).kind, PlaceKind::ApartmentLobby))
            .or_else(|| {
                city_places
                    .iter()
                    .copied()
                    .find(|place_id| world.place(*place_id).kind.supports_people())
            })
            .or_else(|| city_places.first().copied())
            .expect("generated city should have places");
        let player_id = world.ensure_player();
        world.move_player(player_id, start_place_id);
        world.set_current_time(START_TIME);
        world.discover_city(player_id, start_city_id, START_TIME);
        for city_id in world.city_connections(start_city_id) {
            world.discover_city(player_id, city_id, START_TIME);
        }
        world.append_context_entry(
            player_id,
            ContextEntry::System {
                timestamp: START_TIME,
                context: SystemContext::Start,
            },
        );

        Ok(Self {
            state: GameState { world },
            backend,
        })
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    pub fn snapshot(&self) -> UiSnapshot {
        build_ui_snapshot(&self.state)
    }

    pub async fn apply_command(&mut self, command: GameCommand) -> Result<CommandResult> {
        let events = match command {
            GameCommand::TravelTo(destination) => self.travel_to(destination)?,
            GameCommand::OpenDialogue(npc_id) => self.start_dialogue(npc_id)?,
            GameCommand::SubmitDialogueLine(input) => self.submit_dialogue_line(input).await?,
            GameCommand::InspectEntity(entity_id) => vec![self.inspect_entity(entity_id)?],
            GameCommand::Wait(duration) => vec![self.wait_for(duration.max(TimeDelta::ONE_SECOND))],
            GameCommand::LeaveDialogue => vec![self.leave_dialogue().await?],
        };
        Ok(CommandResult {
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
        let mut state = serde_json::from_str::<GameState>(&data)?;
        let player_id = state.world.ensure_player();
        let current_time = state.world.current_time();
        for process_id in state.world.active_dialogue_process_ids(player_id) {
            state.world.end_process(process_id, current_time);
        }
        if state.world.player_place_id(player_id).is_none() {
            let start_city_id = state.world.city_ids()[0];
            let start_place_id = state.world.city_places(start_city_id)[0];
            state.world.move_player(player_id, start_place_id);
        }
        validate_world(&state.world)?;
        self.state = state;
        Ok(())
    }

    async fn submit_dialogue_line(&mut self, input: String) -> Result<Vec<GameEvent>> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let Some(process_id) = active_dialogue_process_id(&self.state) else {
            bail!("You are not talking to anyone right now.");
        };
        let mut events = self.record_dialogue_line(DialogueSpeaker::Player, trimmed.to_string());

        let npc_id = self
            .state
            .world
            .dialogue_npc_id(process_id)
            .expect("active dialogue should include an npc");
        let memory = self
            .state
            .world
            .npc_conversation_memory(npc_id)
            .unwrap_or_default();
        let context = build_npc_dialogue_context(
            &self.state.world,
            current_time(&self.state),
            current_city_id(&self.state),
            &memory,
            process_id,
            trimmed.to_string(),
        )?;

        let response = self.backend.generate_dialogue(&context).await?;
        let text = response.text;

        events.extend(self.record_dialogue_line(DialogueSpeaker::Npc(npc_id), text));
        self.advance_time(DIALOGUE_TIME);
        Ok(events)
    }

    fn travel_to(&mut self, destination_id: PlaceId) -> Result<Vec<GameEvent>> {
        let (resolved_destination_id, route) = self
            .state
            .world
            .place_routes(current_place_id(&self.state))
            .into_iter()
            .find(|(place_id, _)| *place_id == destination_id)
            .ok_or_else(|| anyhow::anyhow!("Selected route is no longer available."))?;
        let travel_time = route.travel_time;

        self.move_player_to(resolved_destination_id);
        self.advance_time(travel_time);
        let player_id = self.player_id();
        self.state.world.record_travel_process(
            player_id,
            resolved_destination_id,
            travel_time,
            current_time(&self.state),
        );
        self.learn_city(current_city_id(&self.state));
        let destination = self.place_summary(resolved_destination_id);
        let context_event = self.push_system_context(
            current_time(&self.state),
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

    fn start_dialogue(&mut self, npc_id: NpcId) -> Result<Vec<GameEvent>> {
        if active_dialogue_process_id(&self.state).is_some() {
            bail!("You are already talking to someone.");
        }
        let is_nearby = self
            .state
            .world
            .place_npcs(current_place_id(&self.state))
            .contains(&npc_id);
        if !is_nearby {
            bail!("That person is no longer here.");
        }
        let player_id = self.player_id();
        self.state.world.start_dialogue_process(
            player_id,
            npc_id,
            current_place_id(&self.state),
            current_time(&self.state),
        );
        let opening_text = format!(
            "What do you want to know about {}?",
            self.state.world.city_name(current_city_id(&self.state))
        );
        let mut events = vec![GameEvent::DialogueStarted { npc_id }];
        events.extend(self.record_dialogue_line(DialogueSpeaker::Npc(npc_id), opening_text));
        Ok(events)
    }

    fn inspect_entity(&self, entity_id: EntityId) -> Result<GameEvent> {
        let is_here = self
            .state
            .world
            .place_entities(current_place_id(&self.state))
            .contains(&entity_id);
        if !is_here {
            bail!("That entity is no longer here.");
        }
        Ok(GameEvent::EntityInspected {
            entity: self.entity_summary(entity_id),
        })
    }

    fn wait_for(&mut self, duration: TimeDelta) -> GameEvent {
        let duration = duration.max(TimeDelta::ONE_SECOND);
        self.advance_time(duration);
        let player_id = self.player_id();
        self.state.world.record_waiting_process(
            player_id,
            current_place_id(&self.state),
            duration,
            current_time(&self.state),
        );
        GameEvent::WaitCompleted {
            duration,
            current_time: current_time(&self.state),
        }
    }

    async fn leave_dialogue(&mut self) -> Result<GameEvent> {
        let Some(process_id) = active_dialogue_process_id(&self.state) else {
            bail!("You are not talking to anyone right now.");
        };
        let npc_id = active_dialogue_npc_id(&self.state)
            .expect("active dialogue should include an npc participant");
        let transcript = self.state.world.dialogue_lines(process_id);
        let summary = self.backend.summarize_memory(&transcript).await?;
        self.state
            .world
            .end_process(process_id, current_time(&self.state));
        self.state
            .world
            .merge_npc_conversation_memory(npc_id, summary);
        Ok(GameEvent::DialogueEnded { npc_id })
    }

    fn push_system_context(&mut self, timestamp: GameTime, context: SystemContext) -> GameEvent {
        let entry = ContextEntry::System { timestamp, context };
        self.state
            .world
            .append_context_entry(self.player_id(), entry.clone());
        GameEvent::ContextAppended { entry }
    }

    fn move_player_to(&mut self, place_id: PlaceId) {
        let player_id = self.player_id();
        self.state.world.move_player(player_id, place_id);
    }

    fn record_dialogue_line(&mut self, speaker: DialogueSpeaker, text: String) -> Vec<GameEvent> {
        let process_id = active_dialogue_process_id(&self.state)
            .expect("dialogue should be active while recording a line");
        let line = DialogueLine {
            timestamp: current_time(&self.state),
            speaker,
            text,
        };
        self.state
            .world
            .append_dialogue_utterance(process_id, self.player_id(), line.clone());
        let entry = ContextEntry::Dialogue(line.clone());
        vec![
            GameEvent::DialogueLineRecorded { line },
            GameEvent::ContextAppended { entry },
        ]
    }

    fn advance_time(&mut self, duration: TimeDelta) {
        let next_time = current_time(&self.state).advance(duration);
        self.state.world.set_current_time(next_time);
    }

    fn learn_city(&mut self, city_id: crate::world::CityId) {
        let player_id = self.player_id();
        self.state
            .world
            .discover_city(player_id, city_id, current_time(&self.state));
        for connected in self.state.world.city_connections(city_id) {
            self.state
                .world
                .discover_city(player_id, connected, current_time(&self.state));
        }
    }

    fn player_id(&self) -> crate::world::PlayerId {
        player_id(&self.state)
    }

    fn place_summary(&self, place_id: PlaceId) -> PlaceSummary {
        build_place_summary(&self.state.world, place_id)
    }

    fn entity_summary(&self, entity_id: EntityId) -> EntitySummary {
        build_entity_summary(&self.state.world, entity_id)
    }
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

    use crate::ai::context::NpcDialogueContext;
    use crate::domain::commands::GameCommand;
    use crate::domain::events::{DialogueLine, GameEvent};
    use crate::domain::memory::ConversationMemory;
    use crate::domain::time::TimeDelta;
    use crate::graph_ecs::WorldEdge;
    use crate::llm::{DialogueResponse, LlmBackend, MockBackend};
    use crate::simulation::{Interactable, UiMode};
    use crate::world::NpcId;

    use super::GameService;

    #[derive(Debug, Clone, Copy)]
    struct FailingSummaryBackend;

    impl LlmBackend for FailingSummaryBackend {
        async fn generate_dialogue(
            &self,
            _context: &NpcDialogueContext,
        ) -> anyhow::Result<DialogueResponse> {
            Ok(DialogueResponse {
                text: "Normal reply.".to_string(),
            })
        }

        async fn summarize_memory(
            &self,
            _transcript: &[DialogueLine],
        ) -> anyhow::Result<ConversationMemory> {
            anyhow::bail!("summary failed")
        }

        fn name(&self) -> &'static str {
            "failing-summary"
        }
    }

    fn nearby_npc_id<B: LlmBackend>(game: &GameService<B>) -> NpcId {
        game.snapshot()
            .interactables
            .into_iter()
            .find_map(|interactable| match interactable {
                Interactable::Talk(actor) => Some(actor.id),
                _ => None,
            })
            .expect("expected a nearby npc")
    }

    async fn open_dialogue_with_nearby_npc<B: LlmBackend>(game: &mut GameService<B>) -> NpcId {
        let npc_id = nearby_npc_id(game);
        game.apply_command(GameCommand::OpenDialogue(npc_id))
            .await
            .unwrap();
        npc_id
    }

    #[tokio::test]
    async fn dialogue_can_be_opened_and_closed_through_typed_commands() {
        let mut game = GameService::new(MockBackend).unwrap();
        open_dialogue_with_nearby_npc(&mut game).await;
        assert_eq!(game.snapshot().mode, UiMode::Dialogue);
        let leave = game
            .apply_command(GameCommand::LeaveDialogue)
            .await
            .unwrap();
        assert!(matches!(
            leave.events.as_slice(),
            [GameEvent::DialogueEnded { .. }]
        ));
    }

    #[tokio::test]
    async fn save_and_load_round_trip() {
        let mut game = GameService::new(MockBackend).unwrap();
        game.apply_command(GameCommand::Wait(TimeDelta::from_seconds(60)))
            .await
            .unwrap();
        game.save(Path::new("/tmp/riggy-test-save.json")).unwrap();

        let mut loaded = GameService::new(MockBackend).unwrap();
        loaded.load(Path::new("/tmp/riggy-test-save.json")).unwrap();
        assert_eq!(
            game.state.world.current_time(),
            loaded.state.world.current_time()
        );
        let loaded_player_id = loaded.state.world.player_id().unwrap();
        let game_player_id = game.state.world.player_id().unwrap();
        assert_eq!(
            game.state.world.player_city_id(game_player_id),
            loaded.state.world.player_city_id(loaded_player_id)
        );
    }

    #[tokio::test]
    async fn load_closes_stale_dialogue_processes() {
        let mut game = GameService::new(MockBackend).unwrap();
        let npc_id = open_dialogue_with_nearby_npc(&mut game).await;
        assert_eq!(game.snapshot().mode, UiMode::Dialogue);
        game.save(Path::new("/tmp/riggy-dialogue-save.json"))
            .unwrap();

        let mut loaded = GameService::new(MockBackend).unwrap();
        loaded
            .load(Path::new("/tmp/riggy-dialogue-save.json"))
            .unwrap();

        assert_eq!(loaded.snapshot().mode, UiMode::Explore);
        assert_eq!(
            loaded
                .state
                .world
                .active_dialogue_npc_id(loaded.player_id()),
            None
        );
        assert!(loaded.snapshot().interactables.iter().any(
            |interactable| matches!(interactable, Interactable::Talk(actor) if actor.id == npc_id)
        ));
    }

    #[tokio::test]
    async fn dialogue_submission_uses_typed_command_path() {
        let mut game = GameService::new(MockBackend).unwrap();
        open_dialogue_with_nearby_npc(&mut game).await;
        let result = game
            .apply_command(GameCommand::SubmitDialogueLine("hello".to_string()))
            .await
            .unwrap();

        assert!(
            result
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DialogueLineRecorded { .. }))
        );
    }

    #[tokio::test]
    async fn leaving_dialogue_persists_conversation_memory() {
        let mut game = GameService::new(MockBackend).unwrap();
        let npc_id = open_dialogue_with_nearby_npc(&mut game).await;
        game.apply_command(GameCommand::SubmitDialogueLine(
            "tell me about work".to_string(),
        ))
        .await
        .unwrap();
        game.apply_command(GameCommand::LeaveDialogue)
            .await
            .unwrap();

        let memory = game.state.world.npc_conversation_memory(npc_id).unwrap();
        assert!(!memory.summary.is_empty());
        assert!(memory.summary.contains("tell me about work"));
    }

    #[tokio::test]
    async fn leaving_dialogue_preserves_session_when_summary_fails() {
        let mut game = GameService::new(FailingSummaryBackend).unwrap();
        let npc_id = open_dialogue_with_nearby_npc(&mut game).await;
        let error = game
            .apply_command(GameCommand::LeaveDialogue)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("summary failed"));
        assert_eq!(
            game.state.world.active_dialogue_npc_id(game.player_id()),
            Some(npc_id)
        );
    }

    #[tokio::test]
    async fn leaving_dialogue_merges_conversation_memory_across_sessions() {
        let mut game = GameService::new(MockBackend).unwrap();
        let npc_id = open_dialogue_with_nearby_npc(&mut game).await;
        game.apply_command(GameCommand::SubmitDialogueLine(
            "tell me about work".to_string(),
        ))
        .await
        .unwrap();
        game.apply_command(GameCommand::LeaveDialogue)
            .await
            .unwrap();

        game.apply_command(GameCommand::OpenDialogue(npc_id))
            .await
            .unwrap();
        game.apply_command(GameCommand::SubmitDialogueLine(
            "tell me about the city".to_string(),
        ))
        .await
        .unwrap();
        game.apply_command(GameCommand::LeaveDialogue)
            .await
            .unwrap();

        let memory = game.state.world.npc_conversation_memory(npc_id).unwrap();
        assert!(memory.summary.contains("tell me about work"));
        assert!(memory.summary.contains("tell me about the city"));
    }

    #[tokio::test]
    async fn load_rejects_invalid_world_snapshot() {
        let mut game = GameService::new(MockBackend).unwrap();
        let npc_id = game.state.world.npc_ids()[0];
        let resident_city_id = game.state.world.npc_resident_city_ids(npc_id)[0];
        let present_edge_id = game
            .state
            .world
            .graph
            .edges_directed(npc_id.0, petgraph::Direction::Incoming)
            .find(|edge| matches!(edge.weight(), WorldEdge::PresentAt))
            .map(|edge| edge.id())
            .expect("npc should have place");
        let other_city_id = game
            .state
            .world
            .city_ids()
            .into_iter()
            .find(|city_id| *city_id != resident_city_id)
            .expect("world should have another city");
        let other_place_id = game.state.world.city_places(other_city_id)[0];
        game.state.world.graph.remove_edge(present_edge_id);
        game.state
            .world
            .graph
            .add_edge(other_place_id.0, npc_id.0, WorldEdge::PresentAt);

        let invalid_path = Path::new("/tmp/riggy-invalid-save.json");
        std::fs::write(invalid_path, to_vec_pretty(&game.state).unwrap()).unwrap();

        let mut loaded = GameService::new(MockBackend).unwrap();
        let err = loaded.load(invalid_path).unwrap_err();
        assert!(err.to_string().contains("world validation failed"));
    }
}
