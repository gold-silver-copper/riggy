pub use crate::domain::records::{
    ContextEntry, DialogueLine, DialogueSpeaker, EntitySummary, GameEvent, PlaceSummary,
    SystemContext,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub events: Vec<GameEvent>,
    pub should_quit: bool,
}
