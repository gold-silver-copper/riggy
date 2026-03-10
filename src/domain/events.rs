use serde::{Deserialize, Serialize};

use crate::world::{EntityId, EntityKind, NpcId, PlaceId, PlaceKind, TransportMode, TravelRoute};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub events: Vec<GameEvent>,
    pub should_quit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    DialogueStarted {
        actor: NpcRef,
    },
    DialogueLineRecorded {
        line: DialogueEventLine,
    },
    DialogueEnded {
        actor: NpcRef,
    },
    TravelCompleted {
        destination: PlaceRef,
        transport_mode: TransportMode,
        route: TravelRoute,
        duration_seconds: u64,
    },
    VehicleEntered {
        entity: EntityRef,
    },
    VehicleExited {
        entity: EntityRef,
    },
    EntityInspected {
        entity: EntityRef,
    },
    WaitCompleted {
        duration_seconds: u64,
        current_time_seconds: u64,
    },
    RelationshipChanged {
        actor: NpcRef,
        disposition: i32,
        note: Option<String>,
    },
    ContextAppended {
        entry: ContextEvent,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcRef {
    pub id: NpcId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceRef {
    pub id: PlaceId,
    pub name: String,
    pub kind: PlaceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRef {
    pub id: EntityId,
    pub name: String,
    pub kind: EntityKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueEventLine {
    pub timestamp_seconds: u64,
    pub speaker: DialogueSpeakerRef,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogueSpeakerRef {
    Player,
    Npc(NpcRef),
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextEvent {
    System {
        timestamp_seconds: u64,
        context: SystemContext,
    },
    Dialogue {
        timestamp_seconds: u64,
        speaker: DialogueSpeakerRef,
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemContext {
    Start,
    Travel {
        destination_id: PlaceId,
        destination_name: String,
        transport_mode: TransportMode,
        duration_seconds: u64,
    },
    Relationship {
        actor_id: NpcId,
        actor_name: String,
        note: String,
    },
    ProposalRejected {
        actor_id: NpcId,
        actor_name: String,
        reason: String,
    },
}

impl SystemContext {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Travel { .. } => "travel",
            Self::Relationship { .. } => "relationship",
            Self::ProposalRejected { .. } => "ai",
        }
    }
}
