use serde::{Deserialize, Serialize};

use crate::domain::time::{GameTime, TimeDelta};
use crate::world::{
    DistrictId, EntityId, EntityKind, NpcId, PlaceId, PlaceKind, TransportMode, TravelRoute,
};

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
        duration: TimeDelta,
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
        duration: TimeDelta,
        current_time: GameTime,
    },
    ContextAppended {
        entry: ContextEvent,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcRef {
    pub id: NpcId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceRef {
    pub id: PlaceId,
    pub district_id: DistrictId,
    pub kind: PlaceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRef {
    pub id: EntityId,
    pub kind: EntityKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueEventLine {
    pub timestamp: GameTime,
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
        timestamp: GameTime,
        context: SystemContext,
    },
    Dialogue {
        timestamp: GameTime,
        speaker: DialogueSpeakerRef,
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemContext {
    Start,
    Travel {
        destination: PlaceRef,
        transport_mode: TransportMode,
        duration: TimeDelta,
    },
}

impl SystemContext {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Travel { .. } => "travel",
        }
    }
}
