use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::events::SystemContext;
use crate::domain::memory::ConversationMemory;
use crate::domain::time::{GameTime, TimeDelta};
use crate::domain::vocab::{Biome, Culture, Economy, NpcArchetype, Occupation};
use crate::world::{CityId, EntityId, EntityKind, NpcId, PlaceId, PlaceKind, TransportMode, World};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameState {
    pub world: World,
    pub clock: GameTime,
    pub player_city_id: CityId,
    pub player_place_id: PlaceId,
    pub occupancy: OccupancyState,
    pub known_city_ids: Vec<CityId>,
    #[serde(default)]
    pub npc_memories: BTreeMap<NpcId, NpcMemoryState>,
    #[serde(default)]
    pub context_feed: Vec<ContextEntry>,
    pub active_dialogue: Option<DialogueSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OccupancyState {
    OnFoot,
    InVehicle(EntityId),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NpcMemoryState {
    #[serde(default)]
    pub memory: ConversationMemory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextEntry {
    pub timestamp: GameTime,
    pub kind: ContextEntryKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContextEntryKind {
    System(SystemContext),
    Dialogue { speaker: Speaker, text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueSession {
    pub npc_id: NpcId,
    pub started_at: GameTime,
    pub transcript: Vec<DialogueLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialogueLine {
    pub speaker: Speaker,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Speaker {
    Player,
    Npc(NpcId),
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSnapshot {
    pub mode: UiMode,
    pub status: PlayerStatusView,
    pub city: CityView,
    pub place: PlaceView,
    pub dialogue_partner: Option<DialoguePartnerView>,
    pub routes: Vec<RouteView>,
    pub interactables: Vec<InteractableOption>,
    pub nearby_actors: Vec<ActorView>,
    pub nearby_cars: Vec<EntityView>,
    pub nearby_entities: Vec<EntityView>,
    pub context_feed: Vec<ContextFeedEntryView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteView {
    pub destination: PlaceView,
    pub route: crate::world::TravelRoute,
    pub travel_time: Option<TimeDelta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractableOption {
    pub target: InteractionTarget,
    pub verb: InteractionVerb,
    pub subject: InteractableSubjectView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerStatusView {
    pub clock: GameTime,
    pub transport_mode: TransportMode,
    pub known_city_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CityView {
    pub name: String,
    pub biome: Biome,
    pub economy: Economy,
    pub culture: Culture,
    pub districts: Vec<DistrictView>,
    pub landmarks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistrictView {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceView {
    pub id: PlaceId,
    pub name: String,
    pub kind: PlaceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialoguePartnerView {
    pub actor: ActorView,
    pub memory: Option<ConversationMemory>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorView {
    pub id: NpcId,
    pub name: String,
    pub occupation: Occupation,
    pub archetype: NpcArchetype,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorRefView {
    pub id: NpcId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractableSubjectView {
    Actor(ActorView),
    Entity(EntityView),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityView {
    pub id: EntityId,
    pub name: String,
    pub kind: EntityKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextFeedEntryView {
    System {
        timestamp: GameTime,
        context: SystemContext,
    },
    Dialogue {
        timestamp: GameTime,
        speaker: DialogueSpeakerView,
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogueSpeakerView {
    Player,
    Npc(ActorRefView),
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Explore,
    Dialogue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionTarget {
    Npc(NpcId),
    Entity(EntityId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionVerb {
    Talk,
    EnterVehicle,
    ExitVehicle,
    Inspect,
}
