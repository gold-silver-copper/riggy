use crate::domain::time::TimeDelta;
use crate::world::{EntityId, NpcId, PlaceId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameCommand {
    TravelTo(PlaceId),
    OpenDialogue(NpcId),
    SubmitDialogueLine(String),
    EnterVehicle(EntityId),
    ExitVehicle,
    InspectEntity(EntityId),
    Wait(TimeDelta),
    LeaveDialogue,
}
