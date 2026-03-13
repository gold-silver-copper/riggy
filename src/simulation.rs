use serde::{Deserialize, Serialize};

use crate::domain::events::{EntitySummary, PlaceSummary};
use crate::domain::memory::ConversationMemory;
use crate::domain::seed::WorldSeed;
use crate::domain::time::{GameTime, TimeDelta};
use crate::domain::vocab::{Biome, Culture, Economy, NpcArchetype, Occupation};
use crate::world::{CityId, DistrictId, LandmarkId, NpcId, World};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameState {
    pub world: World,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSnapshot {
    pub world_seed: WorldSeed,
    pub mode: UiMode,
    pub status: PlayerStatusView,
    pub city: CityView,
    pub place: PlaceSummary,
    pub dialogue_partner: Option<DialoguePartnerView>,
    pub routes: Vec<RouteView>,
    pub interactables: Vec<Interactable>,
    pub context_feed: Vec<crate::domain::records::ContextEntry>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerStatusView {
    pub clock: GameTime,
    pub known_city_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CityView {
    pub id: CityId,
    pub biome: Biome,
    pub economy: Economy,
    pub culture: Culture,
    pub districts: Vec<DistrictId>,
    pub landmarks: Vec<LandmarkId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialoguePartnerView {
    pub actor: ActorView,
    pub memory: Option<ConversationMemory>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorView {
    pub id: NpcId,
    pub occupation: Occupation,
    pub archetype: NpcArchetype,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Explore,
    Dialogue,
}
