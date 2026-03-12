use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::events::{ContextEntry, DialogueLine, EntitySummary, PlaceSummary};
use crate::domain::memory::ConversationMemory;
use crate::domain::seed::WorldSeed;
use crate::domain::time::{GameTime, TimeDelta};
use crate::domain::vocab::{Biome, Culture, Economy, NpcArchetype, Occupation};
use crate::world::{CityId, DistrictId, EntityId, LandmarkId, NpcId, TransportMode, World};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameState {
    pub world: World,
    pub clock: GameTime,
    pub player_city_id: CityId,
    pub player_place_id: crate::world::PlaceId,
    pub occupancy: OccupancyState,
    pub known_city_ids: Vec<CityId>,
    #[serde(default)]
    pub npc_memories: BTreeMap<NpcId, ConversationMemory>,
    #[serde(default)]
    pub context_feed: Vec<ContextEntry>,
    pub active_dialogue: Option<DialogueSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OccupancyState {
    OnFoot,
    InVehicle(EntityId),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueSession {
    pub npc_id: NpcId,
    pub started_at: GameTime,
    pub transcript: Vec<DialogueLine>,
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
    pub context_feed: Vec<ContextEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteView {
    pub destination: PlaceSummary,
    pub route: crate::world::TravelRoute,
    pub travel_time: Option<TimeDelta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interactable {
    Talk(ActorView),
    EnterVehicle(EntitySummary),
    ExitVehicle(EntitySummary),
    Inspect(EntitySummary),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerStatusView {
    pub clock: GameTime,
    pub transport_mode: TransportMode,
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
