use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, bail};

use crate::ai::context::build_npc_dialogue_context_v1;
use crate::ai::policy::ConservativeProposalPolicy;
use crate::ai::validation::{
    ProposalRejectionReason, ProposalValidationContext, RejectedProposal, ValidatedProposal,
    validate_proposals,
};
use crate::app::query::{current_transport_mode, current_vehicle_id, reachable_car_ids};
use crate::app::read_model::build_ui_snapshot;
use crate::domain::commands::GameCommand;
use crate::domain::events::{
    CommandResult, ContextEvent, DialogueEventLine, DialogueSpeakerRef, EntityRef, GameEvent,
    NpcRef, PlaceRef, SystemContext,
};
use crate::domain::relationship::RelationshipMemory;
use crate::llm::LlmBackend;
use crate::simulation::{
    ContextEntry, ContextEntryKind, DialogueLine, DialogueSession, GameState, OccupancyState,
    RelationshipState, Speaker, UiSnapshot,
};
use crate::world::{EntityId, NpcId, PlaceId, PlaceKind, TransportMode, World};

const START_TIME_SECONDS: u64 = 8 * 60 * 60;
const DIALOGUE_SECONDS: u64 = 30;
const RELATIONSHIP_DECAY_AFTER_SECONDS: u64 = 8 * 60 * 60;
#[derive(Debug)]
pub struct GameService<B> {
    state: GameState,
    backend: B,
}

impl<B: LlmBackend> GameService<B> {
    pub fn new(backend: B) -> Result<Self> {
        let seed = 42;
        let world = World::generate(seed, 18);
        validate_world(&world)?;
        let start_city_id = world.city_ids()[0];
        let city_places = world.city_places(start_city_id);
        let start_place_id = city_places
            .iter()
            .copied()
            .find(|place_id| matches!(world.place(*place_id).kind, PlaceKind::ApartmentLobby))
            .or_else(|| {
                city_places.iter().copied().find(|place_id| {
                    world.place(*place_id).kind.supports_people()
                        && (!world.place_cars(*place_id).is_empty()
                            || world
                                .place_routes(*place_id)
                                .iter()
                                .any(|(neighbor_id, _)| {
                                    matches!(world.place(*neighbor_id).kind, PlaceKind::RoadLane)
                                        && !world.place_cars(*neighbor_id).is_empty()
                                }))
                })
            })
            .or_else(|| {
                city_places
                    .iter()
                    .copied()
                    .find(|place_id| world.place(*place_id).kind.supports_people())
            })
            .or_else(|| city_places.first().copied())
            .expect("generated city should have places");
        let known_city_ids = {
            let mut ids = vec![start_city_id];
            ids.extend(world.city_connections(start_city_id));
            ids.sort_unstable();
            ids.dedup();
            ids
        };

        Ok(Self {
            state: GameState {
                world,
                clock_seconds: START_TIME_SECONDS,
                player_city_id: start_city_id,
                player_place_id: start_place_id,
                occupancy: OccupancyState::OnFoot,
                known_city_ids,
                relationships: BTreeMap::new(),
                context_feed: vec![ContextEntry {
                    timestamp_seconds: START_TIME_SECONDS,
                    kind: ContextEntryKind::System(SystemContext::Start),
                }],
                active_dialogue: None,
            },
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
            GameCommand::EnterVehicle(entity_id) => vec![self.enter_vehicle(entity_id)?],
            GameCommand::ExitVehicle => vec![self.exit_vehicle()?],
            GameCommand::InspectEntity(entity_id) => vec![self.inspect_entity(entity_id)?],
            GameCommand::Wait(seconds) => vec![self.wait_for(seconds.max(1))],
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
        let state = serde_json::from_str::<GameState>(&data)?;
        validate_world(&state.world)?;
        self.state = state;
        Ok(())
    }

    async fn submit_dialogue_line(&mut self, input: String) -> Result<Vec<GameEvent>> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let Some(session) = self.state.active_dialogue.as_mut() else {
            bail!("You are not talking to anyone right now.");
        };
        session.transcript.push(DialogueLine {
            speaker: Speaker::Player,
            text: trimmed.to_string(),
        });

        let mut events = self.push_dialogue_context(
            Speaker::Player,
            trimmed.to_string(),
            self.state.clock_seconds,
        );

        let session_snapshot = self
            .state
            .active_dialogue
            .clone()
            .expect("dialogue just confirmed");
        let npc_id = session_snapshot.npc_id;
        let relationship = self.relationship(npc_id).clone();
        let context = build_npc_dialogue_context_v1(
            &self.state.world,
            self.state.clock_seconds,
            self.state.player_city_id,
            &relationship,
            &session_snapshot,
            trimmed.to_string(),
        )?;

        let response = self.backend.generate_dialogue(&context).await?;
        let text = response.text.clone();

        {
            let session = self
                .state
                .active_dialogue
                .as_mut()
                .expect("dialogue should remain active while submitting");
            session.transcript.push(DialogueLine {
                speaker: Speaker::Npc(npc_id),
                text: text.clone(),
            });
        }
        events.extend(self.push_dialogue_context(
            Speaker::Npc(npc_id),
            text,
            self.state.clock_seconds,
        ));
        self.advance_time(DIALOGUE_SECONDS);
        let review = validate_proposals(
            &ConservativeProposalPolicy,
            &ProposalValidationContext {
                active_dialogue_npc_id: self
                    .state
                    .active_dialogue
                    .as_ref()
                    .map(|session| session.npc_id),
                target_npc_id: npc_id,
                target_exists: self.state.world.npc_ids().contains(&npc_id),
                current_disposition: self.relationship(npc_id).disposition,
            },
            response.proposals,
        );
        events.extend(self.record_rejected_proposals(npc_id, review.rejected));
        events.extend(self.apply_validated_proposals(review.accepted));

        Ok(events)
    }

    fn travel_to(&mut self, destination_id: PlaceId) -> Result<Vec<GameEvent>> {
        let place = self.current_place();
        let (resolved_destination_id, route) = self
            .state
            .world
            .place_routes(self.state.player_place_id)
            .into_iter()
            .find(|(place_id, _)| *place_id == destination_id)
            .ok_or_else(|| anyhow::anyhow!("Selected route is no longer available."))?;
        let transport_mode = current_transport_mode(&self.state);
        if transport_mode == TransportMode::Car
            && (!matches!(place.kind, PlaceKind::RoadLane)
                || !matches!(
                    self.state.world.place(resolved_destination_id).kind,
                    PlaceKind::RoadLane
                ))
        {
            bail!("You can only drive along roads while you are in a vehicle.");
        }
        let travel_seconds = route.travel_seconds(transport_mode).ok_or_else(|| {
            anyhow::anyhow!("You cannot use {} on this route.", transport_mode.label())
        })?;

        self.state.player_place_id = resolved_destination_id;
        self.state.player_city_id = self
            .state
            .world
            .place_city_id(resolved_destination_id)
            .expect("place should belong to a city");
        if let Some(vehicle_id) = current_vehicle_id(&self.state) {
            self.state
                .world
                .move_entity(vehicle_id, resolved_destination_id);
        }
        self.advance_time(travel_seconds);
        self.learn_city(self.state.player_city_id);
        let destination_name = self.current_place().name.clone();
        let context_event = self.push_system_context(
            self.state.clock_seconds,
            SystemContext::Travel {
                destination_id: resolved_destination_id,
                destination_name: destination_name.clone(),
                transport_mode,
                duration_seconds: travel_seconds,
            },
        );
        Ok(vec![
            context_event,
            GameEvent::TravelCompleted {
                destination: PlaceRef {
                    id: resolved_destination_id,
                    name: destination_name,
                    kind: self.current_place().kind,
                },
                transport_mode,
                route,
                duration_seconds: travel_seconds,
            },
        ])
    }

    fn start_dialogue(&mut self, npc_id: NpcId) -> Result<Vec<GameEvent>> {
        let is_nearby = self
            .state
            .world
            .place_npcs(self.state.player_place_id)
            .contains(&npc_id);
        if !is_nearby {
            bail!("That person is no longer here.");
        }
        self.state.active_dialogue = Some(DialogueSession {
            npc_id,
            started_at: self.state.clock_seconds,
            transcript: vec![DialogueLine {
                speaker: Speaker::Npc(npc_id),
                text: format!(
                    "What do you want to know about {}?",
                    self.current_city().name
                ),
            }],
        });
        let opening_text = format!(
            "What do you want to know about {}?",
            self.current_city().name
        );
        let mut events = vec![GameEvent::DialogueStarted {
            actor: self.npc_ref(npc_id),
        }];
        events.extend(self.push_dialogue_context(
            Speaker::Npc(npc_id),
            opening_text,
            self.state.clock_seconds,
        ));
        Ok(events)
    }

    fn enter_vehicle(&mut self, entity_id: EntityId) -> Result<GameEvent> {
        if current_vehicle_id(&self.state).is_some() {
            bail!("You are already in a vehicle.");
        }
        if !reachable_car_ids(&self.state).contains(&entity_id) {
            bail!("That vehicle is no longer reachable.");
        }
        let vehicle_place_id = self
            .state
            .world
            .entity_place_id(entity_id)
            .expect("vehicle should always belong to a place");
        if !matches!(
            self.state.world.place(vehicle_place_id).kind,
            PlaceKind::RoadLane
        ) {
            bail!("You can only get into a vehicle that is parked on a road.");
        }
        self.state.player_place_id = vehicle_place_id;
        self.state.player_city_id = self
            .state
            .world
            .place_city_id(vehicle_place_id)
            .expect("place should belong to a city");
        let vehicle = self.state.world.entity(entity_id).clone();
        self.state.occupancy = OccupancyState::InVehicle(entity_id);
        Ok(GameEvent::VehicleEntered {
            entity: EntityRef {
                id: entity_id,
                name: vehicle.name,
                kind: vehicle.kind,
            },
        })
    }

    fn exit_vehicle(&mut self) -> Result<GameEvent> {
        let Some(vehicle_id) = current_vehicle_id(&self.state) else {
            bail!("You are not in a vehicle.");
        };
        let vehicle = self.state.world.entity(vehicle_id).clone();
        self.state.occupancy = OccupancyState::OnFoot;
        Ok(GameEvent::VehicleExited {
            entity: EntityRef {
                id: vehicle_id,
                name: vehicle.name,
                kind: vehicle.kind,
            },
        })
    }

    fn inspect_entity(&self, entity_id: EntityId) -> Result<GameEvent> {
        let is_here = self
            .state
            .world
            .place_entities(self.state.player_place_id)
            .contains(&entity_id);
        if !is_here {
            bail!("That entity is no longer here.");
        }
        let entity = self.state.world.entity(entity_id);
        Ok(GameEvent::EntityInspected {
            entity: EntityRef {
                id: entity_id,
                name: entity.name.clone(),
                kind: entity.kind,
            },
        })
    }

    fn wait_for(&mut self, seconds: u64) -> GameEvent {
        let seconds = seconds.max(1);
        self.advance_time(seconds);
        GameEvent::WaitCompleted {
            duration_seconds: seconds,
            current_time_seconds: self.state.clock_seconds,
        }
    }

    async fn leave_dialogue(&mut self) -> Result<GameEvent> {
        let Some(session) = self.state.active_dialogue.clone() else {
            bail!("You are not talking to anyone right now.");
        };
        let npc_id = session.npc_id;
        let npc_name = self.state.world.npc(npc_id).name.clone();
        let summary = self.backend.summarize_memory(&session).await?;
        let clock_seconds = self.state.clock_seconds;
        self.state.active_dialogue.take();
        let relationship = self.relationship_mut(npc_id);
        relationship.memory.merge_update(summary);
        relationship.last_interaction_at = clock_seconds;
        Ok(GameEvent::DialogueEnded {
            actor: NpcRef {
                id: npc_id,
                name: npc_name,
            },
        })
    }

    fn push_system_context(&mut self, timestamp_seconds: u64, context: SystemContext) -> GameEvent {
        self.state.context_feed.push(ContextEntry {
            timestamp_seconds,
            kind: ContextEntryKind::System(context.clone()),
        });
        GameEvent::ContextAppended {
            entry: ContextEvent::System {
                timestamp_seconds,
                context,
            },
        }
    }

    fn push_dialogue_context(
        &mut self,
        speaker: Speaker,
        text: String,
        timestamp_seconds: u64,
    ) -> Vec<GameEvent> {
        self.state.context_feed.push(ContextEntry {
            timestamp_seconds,
            kind: ContextEntryKind::Dialogue {
                speaker: speaker.clone(),
                text: text.clone(),
            },
        });
        let speaker_ref = self.dialogue_speaker_ref(&speaker);
        vec![
            GameEvent::DialogueLineRecorded {
                line: DialogueEventLine {
                    timestamp_seconds,
                    speaker: speaker_ref.clone(),
                    text: text.clone(),
                },
            },
            GameEvent::ContextAppended {
                entry: ContextEvent::Dialogue {
                    timestamp_seconds,
                    speaker: speaker_ref,
                    text,
                },
            },
        ]
    }

    fn apply_validated_proposals(&mut self, proposals: Vec<ValidatedProposal>) -> Vec<GameEvent> {
        let mut applied = Vec::new();
        for proposal in proposals {
            match proposal {
                ValidatedProposal::NoChange => {}
                ValidatedProposal::RelationshipAdjustment(adjustment) => {
                    let npc_id = adjustment.target_npc_id;
                    let timestamp_seconds = self.state.clock_seconds;
                    let npc_name = self.state.world.npc(npc_id).name.clone();
                    let disposition = {
                        let relationship = self.relationship_mut(npc_id);
                        relationship.disposition =
                            (relationship.disposition + adjustment.delta).clamp(-10, 10);
                        relationship.last_interaction_at = timestamp_seconds;
                        relationship.disposition
                    };
                    if let Some(note) = &adjustment.note {
                        applied.push(self.push_system_context(
                            timestamp_seconds,
                            SystemContext::Relationship {
                                actor_id: npc_id,
                                actor_name: npc_name.clone(),
                                note: note.clone(),
                            },
                        ));
                    }
                    applied.push(GameEvent::RelationshipChanged {
                        actor: NpcRef {
                            id: npc_id,
                            name: npc_name,
                        },
                        disposition,
                        note: adjustment.note,
                    });
                }
            }
        }
        applied
    }

    fn record_rejected_proposals(
        &mut self,
        npc_id: NpcId,
        rejected: Vec<RejectedProposal>,
    ) -> Vec<GameEvent> {
        if rejected.is_empty() {
            return Vec::new();
        }
        let actor_name = if self.state.world.npc_ids().contains(&npc_id) {
            self.state.world.npc(npc_id).name.clone()
        } else {
            format!("npc#{}", npc_id.index())
        };

        rejected
            .into_iter()
            .map(|rejection| {
                self.push_system_context(
                    self.state.clock_seconds,
                    SystemContext::ProposalRejected {
                        actor_id: npc_id,
                        actor_name: actor_name.clone(),
                        reason: render_proposal_rejection_reason(&rejection.reason),
                    },
                )
            })
            .collect()
    }

    fn advance_time(&mut self, seconds: u64) {
        self.state.clock_seconds = self.state.clock_seconds.saturating_add(seconds);
        for relationship in self.state.relationships.values_mut() {
            let idle = self
                .state
                .clock_seconds
                .saturating_sub(relationship.last_interaction_at);
            if idle > RELATIONSHIP_DECAY_AFTER_SECONDS && relationship.disposition > 0 {
                relationship.disposition -= 1;
            }
        }
    }

    fn learn_city(&mut self, city_id: crate::world::CityId) {
        self.state.known_city_ids.push(city_id);
        self.state
            .known_city_ids
            .extend(self.state.world.city_connections(city_id));
        self.state.known_city_ids.sort_unstable();
        self.state.known_city_ids.dedup();
    }

    fn relationship(&self, npc_id: NpcId) -> &RelationshipState {
        self.state
            .relationships
            .get(&npc_id)
            .unwrap_or(&DEFAULT_RELATIONSHIP)
    }

    fn relationship_mut(&mut self, npc_id: NpcId) -> &mut RelationshipState {
        self.state
            .relationships
            .entry(npc_id)
            .or_insert_with(|| RelationshipState {
                disposition: 0,
                memory: RelationshipMemory::default(),
                last_interaction_at: self.state.clock_seconds,
            })
    }

    fn current_city(&self) -> &crate::world::City {
        self.state.world.city(self.state.player_city_id)
    }

    fn current_place(&self) -> &crate::world::Place {
        self.state.world.place(self.state.player_place_id)
    }

    fn npc_ref(&self, npc_id: NpcId) -> NpcRef {
        let npc = self.state.world.npc(npc_id);
        NpcRef {
            id: npc_id,
            name: npc.name.clone(),
        }
    }

    fn dialogue_speaker_ref(&self, speaker: &Speaker) -> DialogueSpeakerRef {
        match speaker {
            Speaker::Player => DialogueSpeakerRef::Player,
            Speaker::Npc(npc_id) => DialogueSpeakerRef::Npc(self.npc_ref(*npc_id)),
            Speaker::System => DialogueSpeakerRef::System,
        }
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

fn render_proposal_rejection_reason(reason: &ProposalRejectionReason) -> String {
    match reason {
        ProposalRejectionReason::NoActiveDialogue => "no active dialogue session".to_string(),
        ProposalRejectionReason::DialogueTargetMismatch => {
            "proposal targeted a different dialogue participant".to_string()
        }
        ProposalRejectionReason::TargetMissing => "proposal target no longer exists".to_string(),
        ProposalRejectionReason::NoMeaningfulChange => {
            "proposal resulted in no meaningful state change".to_string()
        }
        ProposalRejectionReason::DeltaOutOfRange { delta } => {
            format!("relationship delta {delta} is outside policy bounds")
        }
        ProposalRejectionReason::NoteTooLong { len, max } => {
            format!("proposal note length {len} exceeds max {max}")
        }
    }
}

static DEFAULT_RELATIONSHIP: RelationshipState = RelationshipState {
    disposition: 0,
    memory: RelationshipMemory {
        trust_delta_summary: 0,
        known_topics: Vec::new(),
        unresolved_threads: Vec::new(),
        freeform_summary: String::new(),
    },
    last_interaction_at: 0,
};

#[cfg(test)]
mod tests {
    use std::path::Path;

    use petgraph::visit::EdgeRef;
    use serde_json::to_vec_pretty;

    use crate::ai::context::NpcDialogueContextV1;
    use crate::ai::proposals::{AiProposal, RelationshipAdjustmentProposal};
    use crate::domain::commands::GameCommand;
    use crate::domain::events::{GameEvent, SystemContext};
    use crate::domain::relationship::RelationshipMemory;
    use crate::graph_ecs::WorldEdge;
    use crate::llm::{DialogueResponse, LlmBackend, MockBackend};
    use crate::simulation::{InteractionTarget, UiMode};

    use super::GameService;

    #[derive(Debug, Clone, Copy)]
    struct InvalidProposalBackend;

    #[derive(Debug, Clone, Copy)]
    struct FailingSummaryBackend;

    impl LlmBackend for InvalidProposalBackend {
        async fn generate_dialogue(
            &self,
            _context: &NpcDialogueContextV1,
        ) -> anyhow::Result<DialogueResponse> {
            Ok(DialogueResponse {
                text: "I am saying something normal.".to_string(),
                proposals: vec![AiProposal::RelationshipAdjustment(
                    RelationshipAdjustmentProposal {
                        delta: 9,
                        note: "This should be rejected by policy.".to_string(),
                    },
                )],
            })
        }

        async fn summarize_memory(
            &self,
            _session: &crate::simulation::DialogueSession,
        ) -> anyhow::Result<RelationshipMemory> {
            Ok(RelationshipMemory {
                trust_delta_summary: 0,
                known_topics: Vec::new(),
                unresolved_threads: Vec::new(),
                freeform_summary: "Nothing durable happened.".to_string(),
            })
        }

        fn name(&self) -> &'static str {
            "invalid-proposal"
        }
    }

    impl LlmBackend for FailingSummaryBackend {
        async fn generate_dialogue(
            &self,
            _context: &NpcDialogueContextV1,
        ) -> anyhow::Result<DialogueResponse> {
            Ok(DialogueResponse {
                text: "Normal reply.".to_string(),
                proposals: Vec::new(),
            })
        }

        async fn summarize_memory(
            &self,
            _session: &crate::simulation::DialogueSession,
        ) -> anyhow::Result<RelationshipMemory> {
            anyhow::bail!("summary failed")
        }

        fn name(&self) -> &'static str {
            "failing-summary"
        }
    }

    #[tokio::test]
    async fn dialogue_can_be_opened_and_closed_through_typed_commands() {
        let mut game = GameService::new(MockBackend).unwrap();
        let npc_id = game
            .snapshot()
            .interactables
            .into_iter()
            .find_map(|option| match option.target {
                InteractionTarget::Npc(npc_id) => Some(npc_id),
                InteractionTarget::Entity(_) => None,
            })
            .expect("expected a nearby npc");
        game.apply_command(GameCommand::OpenDialogue(npc_id))
            .await
            .unwrap();
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
        game.apply_command(GameCommand::Wait(60)).await.unwrap();
        game.save(Path::new("/tmp/riggy-test-save.json")).unwrap();

        let mut loaded = GameService::new(MockBackend).unwrap();
        loaded.load(Path::new("/tmp/riggy-test-save.json")).unwrap();
        assert_eq!(game.state.clock_seconds, loaded.state.clock_seconds);
        assert_eq!(game.state.player_city_id, loaded.state.player_city_id);
    }

    #[tokio::test]
    async fn dialogue_submission_uses_typed_command_path() {
        let mut game = GameService::new(MockBackend).unwrap();
        let npc_id = game
            .snapshot()
            .interactables
            .into_iter()
            .find_map(|option| match option.target {
                InteractionTarget::Npc(npc_id) => Some(npc_id),
                InteractionTarget::Entity(_) => None,
            })
            .expect("expected a nearby npc");

        game.apply_command(GameCommand::OpenDialogue(npc_id))
            .await
            .unwrap();
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
    async fn leaving_dialogue_persists_structured_relationship_memory() {
        let mut game = GameService::new(MockBackend).unwrap();
        let npc_id = game
            .snapshot()
            .interactables
            .into_iter()
            .find_map(|option| match option.target {
                InteractionTarget::Npc(npc_id) => Some(npc_id),
                InteractionTarget::Entity(_) => None,
            })
            .expect("expected a nearby npc");

        game.apply_command(GameCommand::OpenDialogue(npc_id))
            .await
            .unwrap();
        game.apply_command(GameCommand::SubmitDialogueLine(
            "tell me about work".to_string(),
        ))
        .await
        .unwrap();
        game.apply_command(GameCommand::LeaveDialogue)
            .await
            .unwrap();

        let relationship = game.state.relationships.get(&npc_id).unwrap();
        assert_eq!(relationship.memory.trust_delta_summary, 1);
        assert!(
            relationship
                .memory
                .known_topics
                .contains(&"local work".to_string())
        );
        assert!(
            relationship
                .memory
                .unresolved_threads
                .contains(&"Follow up on possible local work".to_string())
        );
        assert!(!relationship.memory.freeform_summary.is_empty());
    }

    #[tokio::test]
    async fn leaving_dialogue_preserves_session_when_summary_fails() {
        let mut game = GameService::new(FailingSummaryBackend).unwrap();
        let npc_id = game
            .snapshot()
            .interactables
            .into_iter()
            .find_map(|option| match option.target {
                InteractionTarget::Npc(npc_id) => Some(npc_id),
                InteractionTarget::Entity(_) => None,
            })
            .expect("expected a nearby npc");

        game.apply_command(GameCommand::OpenDialogue(npc_id))
            .await
            .unwrap();
        let error = game
            .apply_command(GameCommand::LeaveDialogue)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("summary failed"));
        assert_eq!(
            game.state
                .active_dialogue
                .as_ref()
                .map(|session| session.npc_id),
            Some(npc_id)
        );
    }

    #[tokio::test]
    async fn leaving_dialogue_merges_structured_memory_across_sessions() {
        let mut game = GameService::new(MockBackend).unwrap();
        let npc_id = game
            .snapshot()
            .interactables
            .into_iter()
            .find_map(|option| match option.target {
                InteractionTarget::Npc(npc_id) => Some(npc_id),
                InteractionTarget::Entity(_) => None,
            })
            .expect("expected a nearby npc");

        game.apply_command(GameCommand::OpenDialogue(npc_id))
            .await
            .unwrap();
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

        let relationship = game.state.relationships.get(&npc_id).unwrap();
        assert!(
            relationship
                .memory
                .known_topics
                .contains(&"local work".to_string())
        );
        assert!(
            relationship
                .memory
                .known_topics
                .contains(&"city layout".to_string())
        );
        assert!(
            relationship
                .memory
                .unresolved_threads
                .contains(&"Follow up on possible local work".to_string())
        );
    }

    #[tokio::test]
    async fn load_rejects_invalid_world_snapshot() {
        let mut game = GameService::new(MockBackend).unwrap();
        let npc_id = game.state.world.npc_ids()[0];
        let resident_city_id = game
            .state
            .world
            .graph
            .edges_directed(npc_id.0, petgraph::Direction::Incoming)
            .find(|edge| matches!(edge.weight(), WorldEdge::Resident))
            .map(|edge| crate::graph_ecs::CityId(edge.source()))
            .expect("npc should have resident city");
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

    #[tokio::test]
    async fn invalid_ai_proposals_are_rejected_without_mutating_state() {
        let mut game = GameService::new(InvalidProposalBackend).unwrap();
        let npc_id = game
            .snapshot()
            .interactables
            .into_iter()
            .find_map(|option| match option.target {
                InteractionTarget::Npc(npc_id) => Some(npc_id),
                InteractionTarget::Entity(_) => None,
            })
            .expect("expected a nearby npc");

        game.apply_command(GameCommand::OpenDialogue(npc_id))
            .await
            .unwrap();
        let result = game
            .apply_command(GameCommand::SubmitDialogueLine("hello".to_string()))
            .await
            .unwrap();

        assert!(
            !result
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::RelationshipChanged { .. }))
        );
        assert!(result.events.iter().any(|event| matches!(
            event,
            GameEvent::ContextAppended {
                entry: crate::domain::events::ContextEvent::System {
                    context: SystemContext::ProposalRejected { .. },
                    ..
                }
            }
        )));
        assert_eq!(game.relationship(npc_id).disposition, 0);
    }
}
