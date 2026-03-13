use serde::{Deserialize, Serialize};

use riggy_ontology::time::{GameTime, TimeDelta};

use crate::world::{
    DistrictId, EntityId, EntityKind, NpcId, PlaceId, PlaceKind, TransportMode, TravelRoute,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceSummary {
    pub id: PlaceId,
    pub district_id: DistrictId,
    pub kind: PlaceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitySummary {
    pub id: EntityId,
    pub kind: EntityKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueLine {
    pub timestamp: GameTime,
    pub speaker: DialogueSpeaker,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogueSpeaker {
    Player,
    Npc(NpcId),
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextEntry {
    System {
        timestamp: GameTime,
        context: SystemContext,
    },
    Dialogue(DialogueLine),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemContext {
    Start,
    Travel {
        destination: PlaceSummary,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    DialogueStarted {
        npc_id: NpcId,
    },
    DialogueLineRecorded {
        line: DialogueLine,
    },
    DialogueEnded {
        npc_id: NpcId,
    },
    TravelCompleted {
        destination: PlaceSummary,
        transport_mode: TransportMode,
        route: TravelRoute,
        duration: TimeDelta,
    },
    VehicleEntered {
        entity: EntitySummary,
    },
    VehicleExited {
        entity: EntitySummary,
    },
    EntityInspected {
        entity: EntitySummary,
    },
    WaitCompleted {
        duration: TimeDelta,
        current_time: GameTime,
    },
    ContextAppended {
        entry: ContextEntry,
    },
}
