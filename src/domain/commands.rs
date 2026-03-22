use serde::{Deserialize, Serialize};

use crate::domain::time::TimeDelta;
use crate::world::{ActorId, EntityId, PlaceId};

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
}
