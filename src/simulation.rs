use serde::{Deserialize, Serialize};

use crate::domain::commands::AvailableAction;
use crate::domain::events::{EntitySummary, PlaceSummary};
use crate::domain::seed::WorldSeed;
use crate::domain::time::{GameTime, TimeDelta};
use crate::domain::vocab::{Biome, Culture, Economy, NpcArchetype, Occupation};
use crate::llm::AgentDebugTrace;
use crate::world::{ActorId, CityId, World};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameState {
    pub world: World,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSnapshot {
    pub world_seed: WorldSeed,
    pub focused_actor_id: ActorId,
    pub focused_actor: ActorView,
    pub status: ActorStatusView,
    pub city: CityView,
    pub place: PlaceSummary,
    pub routes: Vec<RouteView>,
    pub interactables: Vec<Interactable>,
    pub available_actions: Vec<AvailableAction>,
    pub context_feed: Vec<crate::domain::records::ContextEntry>,
    pub agent_debug: Vec<AgentDebugSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteView {
    pub destination: PlaceSummary,
    pub route: crate::world::TravelRoute,
    pub travel_time: TimeDelta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interactable {
    Talk(ActorView),
    Inspect(EntitySummary),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorStatusView {
    pub id: ActorId,
    pub clock: GameTime,
    pub known_city_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CityView {
    pub id: CityId,
    pub biome: Biome,
    pub economy: Economy,
    pub culture: Culture,
    pub connected_cities: Vec<CityId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorView {
    pub id: ActorId,
    pub occupation: Occupation,
    pub archetype: NpcArchetype,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDebugSnapshot {
    pub actor: ActorView,
    pub trace: Option<AgentDebugTrace>,
}
