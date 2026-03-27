use serde::{Deserialize, Serialize};

use crate::domain::time::{GameTime, TimeDelta};
use crate::world::{ActorId, CityId, EntityId, EntityKind, PlaceId, PlaceKind, TravelRoute};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceSummary {
    pub id: PlaceId,
    pub city_id: CityId,
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
    Actor(ActorId),
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
    Travel {
        destination: PlaceSummary,
        duration: TimeDelta,
    },
    Inspect {
        entity: EntitySummary,
    },
    Wait {
        duration: TimeDelta,
        current_time: GameTime,
    },
}

impl SystemContext {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Travel { .. } => "travel",
            Self::Inspect { .. } => "inspect",
            Self::Wait { .. } => "wait",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    SpeechLineRecorded {
        line: DialogueLine,
    },
    TravelCompleted {
        actor_id: ActorId,
        destination: PlaceSummary,
        route: TravelRoute,
        duration: TimeDelta,
    },
    EntityInspected {
        actor_id: ActorId,
        entity: EntitySummary,
    },
    WaitCompleted {
        actor_id: ActorId,
        duration: TimeDelta,
        current_time: GameTime,
    },
    ContextAppended {
        entry: ContextEntry,
    },
}
