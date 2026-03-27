use serde::{Deserialize, Serialize};

use crate::domain::time::TimeDelta;
use crate::world::{ActorId, EntityId, PlaceId, TravelRoute};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRequest {
    pub actor_id: ActorId,
    pub action: ActionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    MoveTo { destination: PlaceId },
    Speak { target: ActorId, text: String },
    InspectEntity { entity_id: EntityId },
    Wait { duration: TimeDelta },
    DoNothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AvailableAction {
    MoveTo { destination: PlaceId },
    SpeakTo { target: ActorId },
    InspectEntity { entity_id: EntityId },
    Wait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentAvailableAction {
    MoveTo { destination: PlaceId },
    SpeakTo { target: ActorId },
    InspectEntity { entity_id: EntityId },
    DoNothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPlan {
    pub actor_id: ActorId,
    pub duration: TimeDelta,
    pub action: PlannedAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedAction {
    MoveTo {
        origin: PlaceId,
        destination: PlaceId,
        route: TravelRoute,
    },
    Speak {
        place_id: PlaceId,
        target: ActorId,
        text: String,
    },
    InspectEntity {
        place_id: PlaceId,
        entity_id: EntityId,
    },
    Wait {
        place_id: PlaceId,
    },
    DoNothing {
        place_id: PlaceId,
    },
}
