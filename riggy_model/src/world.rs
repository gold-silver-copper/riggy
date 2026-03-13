use bfo::{BfoClass, RelationKind};
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;
use rand::Rng;
use rand::prelude::IndexedRandom;
use rand_chacha::{ChaCha8Rng, rand_core::SeedableRng};
use serde::{Deserialize, Serialize};

use riggy_ontology::memory::ConversationMemory;
use riggy_ontology::seed::WorldSeed;
use riggy_ontology::time::{GameTime, TimeDelta};
use riggy_ontology::vocab::{Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag};

use crate::invariants::{InvariantViolation, validate_world};
use crate::records::{ContextEntry, DialogueLine, DialogueSpeaker, SystemContext};
pub use crate::graph_ecs::{CityId, EntityId, NpcId, PlaceId, PlayerId, ProcessId};
use crate::graph_ecs::{WorldEdge, WorldGraph, WorldNode, WorldRelation, add_edge, edge_snapshot};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub seed: WorldSeed,
    pub graph: WorldGraph,
}

impl PartialEq for World {
    fn eq(&self, other: &Self) -> bool {
        self.seed == other.seed
            && self.graph.node_count() == other.graph.node_count()
            && self.graph.edge_count() == other.graph.edge_count()
            && self
                .graph
                .node_indices()
                .all(|index| self.graph.node_weight(index) == other.graph.node_weight(index))
            && edge_snapshot(&self.graph) == edge_snapshot(&other.graph)
    }
}

impl Eq for World {}

macro_rules! labeled_enum {
    ($name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }
        }
    };
}

labeled_enum!(TransportMode {
    Walking => "walk",
    Transit => "transit",
    Car => "car",
});

impl TransportMode {
    pub fn next(self) -> Self {
        match self {
            Self::Walking => Self::Transit,
            Self::Transit => Self::Car,
            Self::Car => Self::Walking,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Walking => Self::Car,
            Self::Transit => Self::Walking,
            Self::Car => Self::Transit,
        }
    }
}

labeled_enum!(RouteKind {
    Hallway => "hallway",
    Stairwell => "stairwell",
    Crosswalk => "crosswalk",
    SideStreet => "side street",
    LocalRoad => "local roads",
    ArterialRoad => "arterial road",
    Highway => "highway",
});

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct TravelRoute {
    pub kind: RouteKind,
    pub walking_time: TimeDelta,
    pub transit_time: Option<TimeDelta>,
    pub driving_time: Option<TimeDelta>,
}

impl TravelRoute {
    pub fn travel_time(self, mode: TransportMode) -> Option<TimeDelta> {
        match mode {
            TransportMode::Walking => Some(self.walking_time),
            TransportMode::Transit => self.transit_time,
            TransportMode::Car => self.driving_time,
        }
    }

    pub fn supports(self, mode: TransportMode) -> bool {
        self.travel_time(mode).is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct City {
    pub biome: Biome,
    pub economy: Economy,
    pub culture: Culture,
    pub districts: Vec<District>,
    pub landmarks: Vec<Landmark>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DistrictId {
    pub city_id: CityId,
    pub district_index: u16,
}

impl DistrictId {
    pub fn name(self, seed: WorldSeed) -> String {
        let key = mix_seed(
            seed,
            &[1, self.city_id.index() as u64, self.district_index as u64],
        );
        format!(
            "{} {}",
            DISTRICT_PREFIXES[(key as usize) % DISTRICT_PREFIXES.len()],
            DISTRICT_SUFFIXES[((key >> 16) as usize) % DISTRICT_SUFFIXES.len()]
        )
    }

    pub fn description(self, seed: WorldSeed) -> String {
        let key = mix_seed(
            seed,
            &[2, self.city_id.index() as u64, self.district_index as u64],
        );
        format!(
            "{} with {}",
            DISTRICT_TEXTURES[(key as usize) % DISTRICT_TEXTURES.len()],
            DISTRICT_FUNCTIONS[((key >> 16) as usize) % DISTRICT_FUNCTIONS.len()]
        )
    }
}

impl CityId {
    pub fn name(self, seed: WorldSeed) -> String {
        let key = mix_seed(seed, &[0, self.index() as u64]);
        format!(
            "{}{}",
            CITY_PREFIXES[(key as usize) % CITY_PREFIXES.len()],
            CITY_SUFFIXES[((key >> 16) as usize) % CITY_SUFFIXES.len()]
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LandmarkId {
    pub city_id: CityId,
    pub landmark_index: u16,
}

impl LandmarkId {
    pub fn name(self, seed: WorldSeed) -> String {
        let key = mix_seed(
            seed,
            &[3, self.city_id.index() as u64, self.landmark_index as u64],
        );
        format!(
            "{} {}",
            LANDMARK_PREFIXES[(key as usize) % LANDMARK_PREFIXES.len()],
            LANDMARK_NOUNS[((key >> 16) as usize) % LANDMARK_NOUNS.len()]
        )
    }
}

impl NpcId {
    pub fn name(self, seed: WorldSeed) -> String {
        let key = mix_seed(seed, &[4, self.index() as u64]);
        format!(
            "{} {}",
            NPC_FIRST_NAMES[(key as usize) % NPC_FIRST_NAMES.len()],
            NPC_LAST_NAMES[((key >> 16) as usize) % NPC_LAST_NAMES.len()]
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct District {
    pub id: DistrictId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Landmark {
    pub id: LandmarkId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Place {
    pub district_id: DistrictId,
    pub kind: PlaceKind,
    pub description: String,
}

labeled_enum!(PlaceKind {
    BuildingInterior => "building interior",
    ApartmentLobby => "apartment lobby",
    ApartmentRoom => "apartment room",
    RoadLane => "road lane",
    SidewalkLeft => "left sidewalk",
    SidewalkRight => "right sidewalk",
    StationConcourse => "station concourse",
    StationPlatform => "station platform",
});

impl PlaceKind {
    pub fn supports_people(self) -> bool {
        matches!(
            self,
            Self::BuildingInterior
                | Self::ApartmentLobby
                | Self::ApartmentRoom
                | Self::SidewalkLeft
                | Self::SidewalkRight
                | Self::StationConcourse
                | Self::StationPlatform
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Npc {
    pub home_district: DistrictId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entity {
    pub kind: EntityKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Player;

labeled_enum!(EntityKind {
    Car => "car",
    Gun => "gun",
    Knife => "knife",
    Bag => "bag",
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DependentContinuant {
    Role(RoleNode),
    Disposition(DispositionNode),
    Quality(QualityNode),
}

impl DependentContinuant {
    pub const fn bfo_class(&self) -> BfoClass {
        match self {
            Self::Role(_) => BfoClass::Role,
            Self::Disposition(_) => BfoClass::Disposition,
            Self::Quality(_) => BfoClass::Quality,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleNode {
    pub kind: RoleKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispositionNode {
    pub kind: DispositionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QualityNode {
    pub kind: QualityKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoleKind {
    Occupation(Occupation),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DispositionKind {
    Trait(TraitTag),
    Goal(GoalTag),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QualityKind {
    Archetype(NpcArchetype),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcProfile {
    pub occupation: Occupation,
    pub archetype: NpcArchetype,
    pub traits: Vec<TraitTag>,
    pub goal: GoalTag,
    pub home_district: DistrictId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InformationContent {
    ConversationMemory {
        summary: String,
    },
    DialogueRecord(DialogueLine),
    ContextRecord(ContextEntry),
    CityKnowledge {
        city_id: CityId,
        discovered_at: GameTime,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Occurrent {
    pub kind: OccurrentKind,
    pub started_at: GameTime,
    pub ended_at: Option<GameTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemporalRegion {
    pub current_time: GameTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OccurrentKind {
    Dialogue,
    Travel {
        transport_mode: TransportMode,
        duration: TimeDelta,
    },
    Waiting {
        duration: TimeDelta,
    },
}

impl World {
    pub fn generate(seed: WorldSeed, city_count: usize) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed.raw());
        let target_cities = city_count.clamp(16, 24);
        let mut graph = WorldGraph::default();
        graph.add_node(WorldNode::TemporalRegion(TemporalRegion {
            current_time: GameTime::from_seconds(0),
        }));
        let mut city_ids = Vec::with_capacity(target_cities);
        let mut city_hubs = Vec::with_capacity(target_cities);

        for _ordinal in 0..target_cities {
            let index = graph.add_node(WorldNode::City(City {
                biome: *Biome::ALL.choose(&mut rng).unwrap(),
                economy: *Economy::ALL.choose(&mut rng).unwrap(),
                culture: *Culture::ALL.choose(&mut rng).unwrap(),
                districts: Vec::new(),
                landmarks: Vec::new(),
            }));
            let city_id = CityId(index);
            let district_count = rng.random_range(3..=4);
            let districts = (0..district_count)
                .map(|district_index| {
                    let id = DistrictId {
                        city_id,
                        district_index: district_index as u16,
                    };
                    District { id }
                })
                .collect::<Vec<_>>();
            let landmark_count = rng.random_range(2..=3);
            let landmarks = (0..landmark_count)
                .map(|landmark_index| {
                    let id = LandmarkId {
                        city_id,
                        landmark_index: landmark_index as u16,
                    };
                    Landmark { id }
                })
                .collect::<Vec<_>>();
            match graph.node_weight_mut(index) {
                Some(WorldNode::City(city)) => {
                    city.districts = districts;
                    city.landmarks = landmarks;
                }
                _ => panic!("newly inserted city node missing"),
            }
            city_ids.push(city_id);
        }

        for city_id in &city_ids {
            let city = Self::city_from_graph(&graph, *city_id).clone();
            let mut road_lanes = Vec::new();
            let mut pedestrian_places = Vec::new();
            for (district_index, district) in city.districts.iter().enumerate() {
                let bundle = build_district_bundle(
                    &mut graph,
                    &mut rng,
                    seed,
                    *city_id,
                    district.id,
                    city_id.index() == 0 && district_index == 0,
                    district_index == 0,
                );
                road_lanes.push(bundle.road_id);
                pedestrian_places.extend(bundle.pedestrian_places);
            }

            let hub_district = city
                .districts
                .first()
                .map(|district| district.id)
                .unwrap_or(DistrictId {
                    city_id: *city_id,
                    district_index: 0,
                });
            let station = build_station_bundle(&mut graph, &mut rng, *city_id, hub_district);
            pedestrian_places.push(station.concourse_id);
            pedestrian_places.push(station.platform_id);
            city_hubs.push(station.platform_id);

            for window in road_lanes.windows(2) {
                let route = random_timed_route(
                    &mut rng,
                    RouteKind::LocalRoad,
                    (60, 180),
                    None,
                    Some((20, 60)),
                );
                add_bidirectional_route(&mut graph, window[0].0, window[1].0, route);
            }
            if road_lanes.len() > 2 {
                let a = road_lanes[0];
                let b = *road_lanes.last().unwrap();
                let route = random_timed_route(
                    &mut rng,
                    RouteKind::LocalRoad,
                    (120, 360),
                    Some((60, 180)),
                    Some((30, 120)),
                );
                add_bidirectional_route(&mut graph, a.0, b.0, route);
            }

            for window in pedestrian_places.windows(2) {
                let route =
                    random_timed_route(&mut rng, RouteKind::SideStreet, (20, 90), None, None);
                add_bidirectional_route(&mut graph, window[0].0, window[1].0, route);
            }

            if let Some(station_sidewalk) = pedestrian_places.first().copied() {
                let station_access =
                    random_timed_route(&mut rng, RouteKind::Hallway, (20, 60), None, None);
                add_bidirectional_route(
                    &mut graph,
                    station_sidewalk.0,
                    station.concourse_id.0,
                    station_access,
                );
            }
        }

        for pos in 0..target_cities {
            let next = (pos + 1) % target_cities;
            let route = random_route(&mut rng, true);
            add_bidirectional_route(&mut graph, city_ids[pos].0, city_ids[next].0, route);
            add_bidirectional_route(&mut graph, city_hubs[pos].0, city_hubs[next].0, route);
        }

        for _ in 0..(target_cities / 2) {
            let a = rng.random_range(0..target_cities);
            let mut b = rng.random_range(0..target_cities);
            while b == a {
                b = rng.random_range(0..target_cities);
            }
            let route = random_route(&mut rng, false);
            add_bidirectional_route(&mut graph, city_ids[a].0, city_ids[b].0, route);
            add_bidirectional_route(&mut graph, city_hubs[a].0, city_hubs[b].0, route);
        }

        for city_id in &city_ids {
            spawn_city_npcs(&mut graph, &mut rng, *city_id);
        }

        graph.add_node(WorldNode::Player(Player));

        Self { seed, graph }
    }

    pub fn city(&self, id: CityId) -> &City {
        Self::city_from_graph(&self.graph, id)
    }

    pub fn city_name(&self, id: CityId) -> String {
        id.name(self.seed)
    }

    pub fn npc(&self, id: NpcId) -> &Npc {
        match self.graph.node_weight(id.0) {
            Some(WorldNode::Npc(npc)) => npc,
            _ => panic!("invalid npc id {:?}", id),
        }
    }

    pub fn npc_profile(&self, id: NpcId) -> NpcProfile {
        let npc = self.npc(id);
        let mut occupation = None;
        let mut archetype = None;
        let mut goal = None;
        let mut traits = Vec::new();

        for dependent in collect_incoming(
            &self.graph,
            id.0,
            WorldRelation::Bfo(RelationKind::SpecificallyDependsOn),
            |_, node, _| match node {
                WorldNode::DependentContinuant(node) => Some(node.clone()),
                _ => None,
            },
        ) {
            match dependent {
                DependentContinuant::Role(role) => match role.kind {
                    RoleKind::Occupation(value) => occupation = Some(value),
                },
                DependentContinuant::Disposition(disposition) => match disposition.kind {
                    DispositionKind::Trait(value) => traits.push(value),
                    DispositionKind::Goal(value) => goal = Some(value),
                },
                DependentContinuant::Quality(quality) => match quality.kind {
                    QualityKind::Archetype(value) => archetype = Some(value),
                },
            }
        }

        traits.sort();

        NpcProfile {
            occupation: occupation.expect("npc should have occupation role"),
            archetype: archetype.expect("npc should have archetype quality"),
            traits,
            goal: goal.expect("npc should have goal disposition"),
            home_district: npc.home_district,
        }
    }

    pub fn npc_name(&self, id: NpcId) -> String {
        id.name(self.seed)
    }

    pub fn place(&self, id: PlaceId) -> &Place {
        Self::place_from_graph(&self.graph, id)
    }

    pub fn place_name(&self, id: PlaceId) -> String {
        let place = self.place(id);
        place_name_from_parts(self.seed, id, place.district_id, place.kind)
    }

    pub fn entity(&self, id: EntityId) -> &Entity {
        match self.graph.node_weight(id.0) {
            Some(WorldNode::Entity(entity)) => entity,
            _ => panic!("invalid entity id {:?}", id),
        }
    }

    pub fn entity_name(&self, id: EntityId) -> String {
        let entity = self.entity(id);
        entity_name_from_parts(self.seed, id, entity.kind)
    }

    pub fn validate(&self) -> Vec<InvariantViolation> {
        validate_world(self)
    }

    pub fn bfo_class(&self, index: NodeIndex) -> Option<BfoClass> {
        self.graph.node_weight(index).map(WorldNode::bfo_class)
    }

    pub fn current_time(&self) -> GameTime {
        let index = self
            .current_time_node_index()
            .expect("world graph should contain a current time node");
        let Some(WorldNode::TemporalRegion(region)) = self.graph.node_weight(index) else {
            unreachable!("current time node should always be a temporal region");
        };
        region.current_time
    }

    pub fn set_current_time(&mut self, current_time: GameTime) {
        if let Some(index) = self.current_time_node_index() {
            let Some(WorldNode::TemporalRegion(region)) = self.graph.node_weight_mut(index) else {
                unreachable!("current time node should always be a temporal region");
            };
            region.current_time = current_time;
            return;
        }

        self.graph
            .add_node(WorldNode::TemporalRegion(TemporalRegion { current_time }));
    }

    pub fn player_id(&self) -> Option<PlayerId> {
        collect_node_ids(&self.graph, |index, node| match node {
            WorldNode::Player(_) => Some(PlayerId(index)),
            _ => None,
        })
        .into_iter()
        .next()
    }

    pub fn ensure_player(&mut self) -> PlayerId {
        if let Some(player_id) = self.player_id() {
            return player_id;
        }
        let index = self.graph.add_node(WorldNode::Player(Player));
        PlayerId(index)
    }

    pub fn place_player_ids(&self, place_id: PlaceId) -> Vec<PlayerId> {
        collect_outgoing(
            &self.graph,
            place_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::PresentAt),
            |index, node, _| match node {
                WorldNode::Player(_) => Some(PlayerId(index)),
                _ => None,
            },
        )
    }

    pub fn player_place_id(&self, player_id: PlayerId) -> Option<PlaceId> {
        collect_incoming(
            &self.graph,
            player_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::PresentAt),
            |index, node, _| match node {
                WorldNode::Place(_) => Some(PlaceId(index)),
                _ => None,
            },
        )
        .into_iter()
        .next()
    }

    pub fn player_city_id(&self, player_id: PlayerId) -> Option<CityId> {
        self.player_place_id(player_id)
            .and_then(|place_id| self.place_city_id(place_id))
    }

    pub fn active_dialogue_process_ids(&self, player_id: PlayerId) -> Vec<ProcessId> {
        let mut ids = collect_incoming(
            &self.graph,
            player_id.0,
            WorldRelation::Bfo(RelationKind::HasParticipant),
            |index, node, _| match node {
                WorldNode::Occurrent(Occurrent {
                    kind: OccurrentKind::Dialogue,
                    ended_at: None,
                    ..
                }) => Some(ProcessId(index)),
                _ => None,
            },
        );
        ids.sort_unstable();
        ids
    }

    pub fn active_dialogue_process_id(&self, player_id: PlayerId) -> Option<ProcessId> {
        self.active_dialogue_process_ids(player_id)
            .into_iter()
            .next()
    }

    pub fn active_dialogue_npc_id(&self, player_id: PlayerId) -> Option<NpcId> {
        self.active_dialogue_process_id(player_id)
            .and_then(|process_id| self.dialogue_npc_id(process_id))
    }

    pub fn player_vehicle_id(&self, player_id: PlayerId) -> Option<EntityId> {
        collect_incoming(
            &self.graph,
            player_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::Contains),
            |index, node, edge| match (node, edge) {
                (WorldNode::Entity(_), WorldEdge::ContainsPlayer) => Some(EntityId(index)),
                _ => None,
            },
        )
        .into_iter()
        .next()
    }

    pub fn move_player(&mut self, player_id: PlayerId, place_id: PlaceId) {
        let existing = self
            .graph
            .edges_directed(player_id.0, Incoming)
            .find(|edge| matches!(edge.weight(), WorldEdge::PresentAt))
            .map(|edge| edge.id());
        if let Some(edge_id) = existing {
            self.graph.remove_edge(edge_id);
        }
        add_edge(
            &mut self.graph,
            place_id.0,
            player_id.0,
            WorldEdge::PresentAt,
        );
    }

    pub fn board_player_vehicle(&mut self, player_id: PlayerId, vehicle_id: EntityId) {
        self.leave_player_vehicle(player_id);
        add_edge(
            &mut self.graph,
            vehicle_id.0,
            player_id.0,
            WorldEdge::ContainsPlayer,
        );
    }

    pub fn leave_player_vehicle(&mut self, player_id: PlayerId) {
        let existing = self
            .graph
            .edges_directed(player_id.0, Incoming)
            .find(|edge| matches!(edge.weight(), WorldEdge::ContainsPlayer))
            .map(|edge| edge.id());
        if let Some(edge_id) = existing {
            self.graph.remove_edge(edge_id);
        }
    }

    pub fn city_ids(&self) -> Vec<CityId> {
        collect_node_ids(&self.graph, |index, node| match node {
            WorldNode::City(_) => Some(CityId(index)),
            _ => None,
        })
    }

    pub fn npc_ids(&self) -> Vec<NpcId> {
        collect_node_ids(&self.graph, |index, node| match node {
            WorldNode::Npc(_) => Some(NpcId(index)),
            _ => None,
        })
    }

    pub fn city_connections(&self, city_id: CityId) -> Vec<CityId> {
        collect_outgoing(
            &self.graph,
            city_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::TravelRoute),
            |index, node, _| match node {
                WorldNode::City(_) => Some(CityId(index)),
                _ => None,
            },
        )
    }

    pub fn city_npcs(&self, city_id: CityId) -> Vec<NpcId> {
        Self::city_npcs_from_graph(&self.graph, city_id)
    }

    pub fn city_places(&self, city_id: CityId) -> Vec<PlaceId> {
        Self::city_places_from_graph(&self.graph, city_id)
    }

    pub fn place_routes(&self, place_id: PlaceId) -> Vec<(PlaceId, TravelRoute)> {
        collect_outgoing(
            &self.graph,
            place_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::TravelRoute),
            |index, node, edge| match (node, edge) {
                (WorldNode::Place(_), WorldEdge::TravelRoute(route)) => {
                    Some((PlaceId(index), *route))
                }
                _ => None,
            },
        )
    }

    pub fn place_npcs(&self, place_id: PlaceId) -> Vec<NpcId> {
        collect_outgoing(
            &self.graph,
            place_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::PresentAt),
            |index, node, _| match node {
                WorldNode::Npc(_) => Some(NpcId(index)),
                _ => None,
            },
        )
    }

    pub fn place_entities(&self, place_id: PlaceId) -> Vec<EntityId> {
        collect_outgoing(
            &self.graph,
            place_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::Contains),
            |index, node, _| match node {
                WorldNode::Entity(_) => Some(EntityId(index)),
                _ => None,
            },
        )
    }

    pub fn place_cars(&self, place_id: PlaceId) -> Vec<EntityId> {
        self.place_entities(place_id)
            .into_iter()
            .filter(|entity_id| matches!(self.entity(*entity_id).kind, EntityKind::Car))
            .collect()
    }

    pub fn place_city_ids(&self, place_id: PlaceId) -> Vec<CityId> {
        collect_incoming(
            &self.graph,
            place_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::Contains),
            |index, node, _| match node {
                WorldNode::City(_) => Some(CityId(index)),
                _ => None,
            },
        )
    }

    pub fn entity_place_id(&self, entity_id: EntityId) -> Option<PlaceId> {
        self.entity_container_place_ids(entity_id)
            .into_iter()
            .next()
    }

    pub fn entity_container_place_ids(&self, entity_id: EntityId) -> Vec<PlaceId> {
        collect_incoming(
            &self.graph,
            entity_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::Contains),
            |index, node, _| match node {
                WorldNode::Place(_) => Some(PlaceId(index)),
                _ => None,
            },
        )
    }

    pub fn npc_resident_city_ids(&self, npc_id: NpcId) -> Vec<CityId> {
        collect_incoming(
            &self.graph,
            npc_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::ResidentOf),
            |index, node, _| match node {
                WorldNode::City(_) => Some(CityId(index)),
                _ => None,
            },
        )
    }

    pub fn npc_present_place_ids(&self, npc_id: NpcId) -> Vec<PlaceId> {
        collect_incoming(
            &self.graph,
            npc_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::PresentAt),
            |index, node, _| match node {
                WorldNode::Place(_) => Some(PlaceId(index)),
                _ => None,
            },
        )
    }

    pub fn npc_conversation_memory(&self, npc_id: NpcId) -> Option<ConversationMemory> {
        collect_incoming(
            &self.graph,
            npc_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::IsAbout),
            |_, node, _| match node {
                WorldNode::InformationContent(InformationContent::ConversationMemory {
                    summary,
                }) => Some(ConversationMemory {
                    summary: summary.clone(),
                }),
                _ => None,
            },
        )
        .into_iter()
        .next()
    }

    pub fn merge_npc_conversation_memory(&mut self, npc_id: NpcId, update: ConversationMemory) {
        let update = update.normalized();
        if update.is_empty() {
            return;
        }

        if let Some(node_id) = self.graph.node_indices().find(|index| {
            matches!(
                self.graph.node_weight(*index),
                Some(WorldNode::InformationContent(
                    InformationContent::ConversationMemory { .. }
                ))
            ) && self
                .graph
                .edges_connecting(*index, npc_id.0)
                .any(|edge| matches!(edge.weight(), WorldEdge::IsAbout))
        }) {
            let Some(WorldNode::InformationContent(info)) = self.graph.node_weight_mut(node_id)
            else {
                unreachable!("conversation memory node should exist");
            };
            let InformationContent::ConversationMemory { summary } = info else {
                unreachable!("conversation memory node should exist");
            };
            let mut memory = ConversationMemory {
                summary: std::mem::take(summary),
            };
            memory.merge_update(update);
            *summary = memory.summary;
            return;
        }

        let index = self.graph.add_node(WorldNode::InformationContent(
            InformationContent::ConversationMemory {
                summary: update.summary,
            },
        ));
        add_edge(&mut self.graph, index, npc_id.0, WorldEdge::IsAbout);
    }

    pub fn discovered_city_ids(&self, player_id: PlayerId) -> Vec<CityId> {
        let mut ids = collect_incoming(
            &self.graph,
            player_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::IsAbout),
            |_, node, _| match node {
                WorldNode::InformationContent(InformationContent::CityKnowledge {
                    city_id,
                    ..
                }) => Some(*city_id),
                _ => None,
            },
        )
        .into_iter()
        .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub fn discover_city(&mut self, player_id: PlayerId, city_id: CityId, discovered_at: GameTime) {
        let exists = self.graph.node_indices().any(|index| {
            matches!(
                self.graph.node_weight(index),
                Some(WorldNode::InformationContent(InformationContent::CityKnowledge { city_id: existing, .. }))
                    if *existing == city_id
            ) && self
                .graph
                .edges_connecting(index, player_id.0)
                .any(|edge| matches!(edge.weight(), WorldEdge::IsAbout))
        });
        if exists {
            return;
        }
        let info_id = self.graph.add_node(WorldNode::InformationContent(
            InformationContent::CityKnowledge {
                city_id,
                discovered_at,
            },
        ));
        add_edge(&mut self.graph, info_id, player_id.0, WorldEdge::IsAbout);
        add_edge(&mut self.graph, info_id, city_id.0, WorldEdge::IsAbout);
    }

    pub fn dialogue_npc_id(&self, process_id: ProcessId) -> Option<NpcId> {
        collect_outgoing(
            &self.graph,
            process_id.0,
            WorldRelation::Bfo(RelationKind::HasParticipant),
            |index, node, _| match node {
                WorldNode::Npc(_) => Some(NpcId(index)),
                _ => None,
            },
        )
        .into_iter()
        .next()
    }

    pub fn dialogue_lines(&self, process_id: ProcessId) -> Vec<DialogueLine> {
        let mut lines = collect_outgoing(
            &self.graph,
            process_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::HasOutput),
            |index, node, _| match node {
                WorldNode::InformationContent(InformationContent::DialogueRecord(line)) => {
                    Some((index.index(), line.clone()))
                }
                _ => None,
            },
        );
        lines.sort_by_key(|(index, line)| (line.timestamp, *index));
        lines.into_iter().map(|(_, line)| line).collect()
    }

    pub fn start_dialogue_process(
        &mut self,
        player_id: PlayerId,
        npc_id: NpcId,
        place_id: PlaceId,
        started_at: GameTime,
    ) -> ProcessId {
        let index = self.graph.add_node(WorldNode::Occurrent(Occurrent {
            kind: OccurrentKind::Dialogue,
            started_at,
            ended_at: None,
        }));
        add_edge(
            &mut self.graph,
            index,
            player_id.0,
            WorldEdge::HasParticipant,
        );
        add_edge(&mut self.graph, index, npc_id.0, WorldEdge::HasParticipant);
        add_edge(&mut self.graph, index, place_id.0, WorldEdge::OccursIn);
        ProcessId(index)
    }

    pub fn append_dialogue_utterance(
        &mut self,
        process_id: ProcessId,
        player_id: PlayerId,
        line: DialogueLine,
    ) {
        let speaker = line.speaker;
        let info_id = self.graph.add_node(WorldNode::InformationContent(
            InformationContent::DialogueRecord(line),
        ));
        add_edge(&mut self.graph, info_id, player_id.0, WorldEdge::IsAbout);
        add_edge(&mut self.graph, process_id.0, info_id, WorldEdge::HasOutput);
        add_edge(&mut self.graph, info_id, process_id.0, WorldEdge::IsAbout);
        if let DialogueSpeaker::Npc(npc_id) = speaker {
            add_edge(&mut self.graph, info_id, npc_id.0, WorldEdge::IsAbout);
        }
    }

    pub fn append_context_entry(&mut self, player_id: PlayerId, entry: ContextEntry) {
        let info_id = self.graph.add_node(WorldNode::InformationContent(
            InformationContent::ContextRecord(entry.clone()),
        ));
        add_edge(&mut self.graph, info_id, player_id.0, WorldEdge::IsAbout);
        match entry {
            ContextEntry::System {
                context: SystemContext::Travel { destination, .. },
                ..
            } => add_edge(
                &mut self.graph,
                info_id,
                destination.id.0,
                WorldEdge::IsAbout,
            ),
            ContextEntry::System {
                context: SystemContext::Start,
                ..
            }
            | ContextEntry::Dialogue(_) => {}
        }
    }

    pub fn recent_context_entries(&self, player_id: PlayerId, limit: usize) -> Vec<ContextEntry> {
        let mut entries = collect_incoming(
            &self.graph,
            player_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::IsAbout),
            |_, node, _| match node {
                WorldNode::InformationContent(InformationContent::ContextRecord(entry)) => {
                    Some(entry.clone())
                }
                WorldNode::InformationContent(InformationContent::DialogueRecord(line)) => {
                    Some(ContextEntry::Dialogue(line.clone()))
                }
                _ => None,
            },
        );
        entries.sort_by_key(context_entry_timestamp);
        let len = entries.len();
        entries
            .into_iter()
            .skip(len.saturating_sub(limit))
            .collect()
    }

    pub fn end_process(&mut self, process_id: ProcessId, ended_at: GameTime) {
        let Some(WorldNode::Occurrent(process)) = self.graph.node_weight_mut(process_id.0) else {
            panic!("invalid process id {:?}", process_id);
        };
        process.ended_at = Some(ended_at);
    }

    pub fn record_travel_process(
        &mut self,
        player_id: PlayerId,
        destination_id: PlaceId,
        transport_mode: TransportMode,
        duration: TimeDelta,
        ended_at: GameTime,
    ) -> ProcessId {
        let started_at =
            GameTime::from_seconds(ended_at.seconds().saturating_sub(duration.seconds()));
        let index = self.graph.add_node(WorldNode::Occurrent(Occurrent {
            kind: OccurrentKind::Travel {
                transport_mode,
                duration,
            },
            started_at,
            ended_at: Some(ended_at),
        }));
        add_edge(
            &mut self.graph,
            index,
            player_id.0,
            WorldEdge::HasParticipant,
        );
        add_edge(
            &mut self.graph,
            index,
            destination_id.0,
            WorldEdge::OccursIn,
        );
        ProcessId(index)
    }

    pub fn record_waiting_process(
        &mut self,
        player_id: PlayerId,
        place_id: PlaceId,
        duration: TimeDelta,
        ended_at: GameTime,
    ) -> ProcessId {
        let started_at =
            GameTime::from_seconds(ended_at.seconds().saturating_sub(duration.seconds()));
        let index = self.graph.add_node(WorldNode::Occurrent(Occurrent {
            kind: OccurrentKind::Waiting { duration },
            started_at,
            ended_at: Some(ended_at),
        }));
        add_edge(
            &mut self.graph,
            index,
            player_id.0,
            WorldEdge::HasParticipant,
        );
        add_edge(&mut self.graph, index, place_id.0, WorldEdge::OccursIn);
        ProcessId(index)
    }

    pub fn move_entity(&mut self, entity_id: EntityId, place_id: PlaceId) {
        let existing = self
            .graph
            .edges_directed(entity_id.0, Incoming)
            .find(|edge| matches!(edge.weight(), WorldEdge::ContainsEntity))
            .map(|edge| edge.id());
        if let Some(edge_id) = existing {
            self.graph.remove_edge(edge_id);
        }
        add_edge(
            &mut self.graph,
            place_id.0,
            entity_id.0,
            WorldEdge::ContainsEntity,
        );
    }

    pub fn place_city_id(&self, place_id: PlaceId) -> Option<CityId> {
        self.place_city_ids(place_id).into_iter().next()
    }

    fn city_from_graph(graph: &WorldGraph, id: CityId) -> &City {
        match graph.node_weight(id.0) {
            Some(WorldNode::City(city)) => city,
            _ => panic!("invalid city id {:?}", id),
        }
    }

    fn place_from_graph(graph: &WorldGraph, id: PlaceId) -> &Place {
        match graph.node_weight(id.0) {
            Some(WorldNode::Place(place)) => place,
            _ => panic!("invalid place id {:?}", id),
        }
    }

    fn city_npcs_from_graph(graph: &WorldGraph, city_id: CityId) -> Vec<NpcId> {
        collect_outgoing(
            graph,
            city_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::ResidentOf),
            |index, node, _| match node {
                WorldNode::Npc(_) => Some(NpcId(index)),
                _ => None,
            },
        )
    }

    fn city_places_from_graph(graph: &WorldGraph, city_id: CityId) -> Vec<PlaceId> {
        collect_outgoing(
            graph,
            city_id.0,
            WorldRelation::Riggy(riggy_ontology::relation::RiggyRelation::Contains),
            |index, node, _| match node {
                WorldNode::Place(_) => Some(PlaceId(index)),
                _ => None,
            },
        )
    }

    fn current_time_node_index(&self) -> Option<NodeIndex> {
        collect_node_ids(&self.graph, |index, node| match node {
            WorldNode::TemporalRegion(_) => Some(index),
            _ => None,
        })
        .into_iter()
        .next()
    }
}

fn collect_node_ids<T>(
    graph: &WorldGraph,
    map: impl Fn(NodeIndex, &WorldNode) -> Option<T>,
) -> Vec<T> {
    graph
        .node_indices()
        .filter_map(|index| graph.node_weight(index).and_then(|node| map(index, node)))
        .collect()
}

fn collect_outgoing<T>(
    graph: &WorldGraph,
    source: NodeIndex,
    relation: WorldRelation,
    map: impl Fn(NodeIndex, &WorldNode, &WorldEdge) -> Option<T>,
) -> Vec<T> {
    graph
        .edges_directed(source, Outgoing)
        .filter(|edge| edge.weight().relation() == relation)
        .filter_map(|edge| {
            graph
                .node_weight(edge.target())
                .and_then(|node| map(edge.target(), node, edge.weight()))
        })
        .collect()
}

fn collect_incoming<T>(
    graph: &WorldGraph,
    target: NodeIndex,
    relation: WorldRelation,
    map: impl Fn(NodeIndex, &WorldNode, &WorldEdge) -> Option<T>,
) -> Vec<T> {
    graph
        .edges_directed(target, Incoming)
        .filter(|edge| edge.weight().relation() == relation)
        .filter_map(|edge| {
            graph
                .node_weight(edge.source())
                .and_then(|node| map(edge.source(), node, edge.weight()))
        })
        .collect()
}

fn context_entry_timestamp(entry: &ContextEntry) -> GameTime {
    match entry {
        ContextEntry::System { timestamp, .. } => *timestamp,
        ContextEntry::Dialogue(line) => line.timestamp,
    }
}

pub fn place_name_from_parts(
    seed: WorldSeed,
    id: PlaceId,
    district_id: DistrictId,
    kind: PlaceKind,
) -> String {
    let district_name = district_id.name(seed);
    match kind {
        PlaceKind::BuildingInterior => format!(
            "{} {}",
            district_name,
            PLACE_INTERIOR_KINDS
                [(mix_seed(seed, &[6, id.index() as u64]) as usize) % PLACE_INTERIOR_KINDS.len()]
        ),
        PlaceKind::ApartmentLobby => format!("{} Apartments Lobby", district_name),
        PlaceKind::ApartmentRoom => format!(
            "{} Apartments {}",
            district_name,
            APARTMENT_ROOM_LABELS
                [(mix_seed(seed, &[7, id.index() as u64]) as usize) % APARTMENT_ROOM_LABELS.len()]
        ),
        PlaceKind::RoadLane => format!(
            "{} {}",
            district_name,
            PLACE_STREET_KINDS
                [(mix_seed(seed, &[8, id.index() as u64]) as usize) % PLACE_STREET_KINDS.len()]
        ),
        PlaceKind::SidewalkLeft => format!("{} left sidewalk", district_name),
        PlaceKind::SidewalkRight => format!("{} right sidewalk", district_name),
        PlaceKind::StationConcourse => {
            format!("{} Central Concourse", district_id.city_id.name(seed))
        }
        PlaceKind::StationPlatform => {
            format!("{} Platform Level", district_id.city_id.name(seed))
        }
    }
}

pub fn entity_name_from_parts(seed: WorldSeed, id: EntityId, kind: EntityKind) -> String {
    match kind {
        EntityKind::Car => format!(
            "{} {}",
            VEHICLE_PREFIXES
                [(mix_seed(seed, &[9, id.index() as u64]) as usize) % VEHICLE_PREFIXES.len()],
            VEHICLE_MODELS
                [((mix_seed(seed, &[9, id.index() as u64]) >> 16) as usize) % VEHICLE_MODELS.len()]
        ),
        EntityKind::Gun => GUN_NAMES
            [(mix_seed(seed, &[10, id.index() as u64]) as usize) % GUN_NAMES.len()]
        .to_string(),
        EntityKind::Knife => KNIFE_NAMES
            [(mix_seed(seed, &[11, id.index() as u64]) as usize) % KNIFE_NAMES.len()]
        .to_string(),
        EntityKind::Bag => BAG_NAMES
            [(mix_seed(seed, &[12, id.index() as u64]) as usize) % BAG_NAMES.len()]
        .to_string(),
    }
}

fn add_place(
    graph: &mut WorldGraph,
    city_id: CityId,
    district_id: DistrictId,
    kind: PlaceKind,
    description: String,
) -> PlaceId {
    let index = graph.add_node(WorldNode::Place(Place {
        district_id,
        kind,
        description,
    }));
    let place_id = PlaceId(index);
    add_edge(graph, city_id.0, place_id.0, WorldEdge::ContainsPlace);
    place_id
}

fn add_connected_place(
    graph: &mut WorldGraph,
    rng: &mut ChaCha8Rng,
    city_id: CityId,
    district_id: DistrictId,
    from: PlaceId,
    kind: PlaceKind,
    description: String,
    route_kind: RouteKind,
    walking: (u32, u32),
) -> PlaceId {
    let place_id = add_place(graph, city_id, district_id, kind, description);
    add_bidirectional_route(
        graph,
        from.0,
        place_id.0,
        random_timed_route(rng, route_kind, walking, None, None),
    );
    place_id
}

fn add_bidirectional_route(
    graph: &mut WorldGraph,
    from: petgraph::stable_graph::NodeIndex,
    to: petgraph::stable_graph::NodeIndex,
    route: TravelRoute,
) {
    add_edge(graph, from, to, WorldEdge::TravelRoute(route));
    add_edge(graph, to, from, WorldEdge::TravelRoute(route));
}

fn add_entity(graph: &mut WorldGraph, kind: EntityKind) -> EntityId {
    let index = graph.add_node(WorldNode::Entity(Entity { kind }));
    EntityId(index)
}

fn add_entity_to_place(graph: &mut WorldGraph, place_id: PlaceId, kind: EntityKind) -> EntityId {
    let entity_id = add_entity(graph, kind);
    add_edge(graph, place_id.0, entity_id.0, WorldEdge::ContainsEntity);
    entity_id
}

struct DistrictBundle {
    road_id: PlaceId,
    pedestrian_places: Vec<PlaceId>,
}

struct StationBundle {
    concourse_id: PlaceId,
    platform_id: PlaceId,
}

fn build_district_bundle(
    graph: &mut WorldGraph,
    rng: &mut ChaCha8Rng,
    seed: WorldSeed,
    city_id: CityId,
    district_id: DistrictId,
    is_starting_apartment_district: bool,
    is_primary_district: bool,
) -> DistrictBundle {
    let district_name = district_id.name(seed);
    let road_id = add_place(
        graph,
        city_id,
        district_id,
        PlaceKind::RoadLane,
        format!(
            "A vehicle lane in {} where deliveries, rideshares, and through-traffic stack up.",
            district_name
        ),
    );
    let left_sidewalk_id = add_connected_place(
        graph,
        rng,
        city_id,
        district_id,
        road_id,
        PlaceKind::SidewalkLeft,
        format!(
            "The left-side sidewalk in {} with storefront windows, signs, and steady foot traffic.",
            district_name
        ),
        RouteKind::Crosswalk,
        (8, 20),
    );
    let right_sidewalk_id = add_connected_place(
        graph,
        rng,
        city_id,
        district_id,
        road_id,
        PlaceKind::SidewalkRight,
        format!(
            "The right-side sidewalk in {} where bus stops, benches, and curb cuts slow the flow.",
            district_name
        ),
        RouteKind::Crosswalk,
        (8, 20),
    );
    add_bidirectional_route(
        graph,
        left_sidewalk_id.0,
        right_sidewalk_id.0,
        random_timed_route(rng, RouteKind::Crosswalk, (15, 35), None, None),
    );

    let building_id = add_connected_place(
        graph,
        rng,
        city_id,
        district_id,
        left_sidewalk_id,
        PlaceKind::BuildingInterior,
        format!(
            "An interior space in {} where people slow down, talk longer, and watch who comes through.",
            district_name
        ),
        RouteKind::Hallway,
        (8, 20),
    );

    let mut pedestrian_places = vec![left_sidewalk_id, right_sidewalk_id, building_id];
    if is_starting_apartment_district {
        pedestrian_places.extend(add_apartment_cluster(
            graph,
            rng,
            city_id,
            district_id,
            left_sidewalk_id,
            &district_name,
        ));
    }

    if is_primary_district || rng.random_bool(0.55) {
        add_entity_to_place(graph, road_id, EntityKind::Car);
    }
    if rng.random_bool(0.18) {
        let entity_kind = if rng.random_bool(0.5) {
            EntityKind::Knife
        } else {
            EntityKind::Bag
        };
        let sidewalk_target = if rng.random_bool(0.5) {
            left_sidewalk_id
        } else {
            right_sidewalk_id
        };
        add_entity_to_place(graph, sidewalk_target, entity_kind);
    }

    DistrictBundle {
        road_id,
        pedestrian_places,
    }
}

fn add_apartment_cluster(
    graph: &mut WorldGraph,
    rng: &mut ChaCha8Rng,
    city_id: CityId,
    district_id: DistrictId,
    sidewalk_id: PlaceId,
    district_name: &str,
) -> Vec<PlaceId> {
    let lobby_id = add_connected_place(
        graph,
        rng,
        city_id,
        district_id,
        sidewalk_id,
        PlaceKind::ApartmentLobby,
        format!(
            "A modest apartment lobby in {} with mailboxes, a buzzer panel, and scuffed tile from years of foot traffic.",
            district_name
        ),
        RouteKind::Hallway,
        (6, 14),
    );

    let mut places = vec![lobby_id];
    for _ in 0..4 {
        let room_id = add_connected_place(
            graph,
            rng,
            city_id,
            district_id,
            lobby_id,
            PlaceKind::ApartmentRoom,
            format!(
                "A small apartment unit in {} with a narrow kitchen, thin walls, and just enough space to disappear for a while.",
                district_name
            ),
            RouteKind::Hallway,
            (4, 12),
        );
        places.push(room_id);
    }
    places
}

fn build_station_bundle(
    graph: &mut WorldGraph,
    rng: &mut ChaCha8Rng,
    city_id: CityId,
    district_id: DistrictId,
) -> StationBundle {
    let concourse_id = add_place(
        graph,
        city_id,
        district_id,
        PlaceKind::StationConcourse,
        "A loud indoor concourse full of departure boards, kiosks, and hurried transfers."
            .to_string(),
    );
    let platform_id = add_connected_place(
        graph,
        rng,
        city_id,
        district_id,
        concourse_id,
        PlaceKind::StationPlatform,
        "Open-air platforms and curbside bays where regional departures actually leave."
            .to_string(),
        RouteKind::Stairwell,
        (18, 45),
    );
    StationBundle {
        concourse_id,
        platform_id,
    }
}

fn spawn_city_npcs(graph: &mut WorldGraph, rng: &mut ChaCha8Rng, city_id: CityId) {
    let district_ids = World::city_from_graph(graph, city_id)
        .districts
        .iter()
        .map(|district| district.id)
        .collect::<Vec<_>>();
    let mut possible_places = World::city_places_from_graph(graph, city_id)
        .into_iter()
        .filter(|place_id| {
            World::place_from_graph(graph, *place_id)
                .kind
                .supports_people()
        })
        .collect::<Vec<_>>();
    if city_id.index() == 0 {
        if let Some(lobby_id) = possible_places.iter().copied().find(|place_id| {
            matches!(
                World::place_from_graph(graph, *place_id).kind,
                PlaceKind::ApartmentLobby
            )
        }) {
            possible_places.retain(|place_id| *place_id != lobby_id);
            possible_places.insert(0, lobby_id);
        }
    }

    let npc_count = rng.random_range(3..=5);
    for npc_offset in 0..npc_count {
        let mut personality_traits = TraitTag::ALL
            .choose_multiple(rng, 2)
            .copied()
            .collect::<Vec<_>>();
        personality_traits.sort();
        let occupation = *Occupation::ALL.choose(rng).unwrap();
        let archetype = *NpcArchetype::ALL.choose(rng).unwrap();
        let goal = *GoalTag::ALL.choose(rng).unwrap();
        let npc = Npc {
            home_district: *district_ids.choose(rng).unwrap(),
        };
        let index = graph.add_node(WorldNode::Npc(npc.clone()));
        let npc_id = NpcId(index);
        add_edge(graph, city_id.0, npc_id.0, WorldEdge::Resident);
        add_npc_dependent_continuants(
            graph,
            npc_id,
            occupation,
            archetype,
            goal,
            &personality_traits,
        );
        if let Some(place_id) = possible_places
            .get(npc_offset % possible_places.len())
            .copied()
        {
            add_edge(graph, place_id.0, npc_id.0, WorldEdge::PresentAt);
        }
    }
}

fn add_npc_dependent_continuants(
    graph: &mut WorldGraph,
    npc_id: NpcId,
    occupation: Occupation,
    archetype: NpcArchetype,
    goal: GoalTag,
    traits: &[TraitTag],
) {
    let occupation_id = graph.add_node(WorldNode::DependentContinuant(DependentContinuant::Role(
        RoleNode {
            kind: RoleKind::Occupation(occupation),
        },
    )));
    add_edge(
        graph,
        occupation_id,
        npc_id.0,
        WorldEdge::SpecificallyDependsOn,
    );
    add_edge(graph, occupation_id, npc_id.0, WorldEdge::InheresIn);

    let archetype_id = graph.add_node(WorldNode::DependentContinuant(
        DependentContinuant::Quality(QualityNode {
            kind: QualityKind::Archetype(archetype),
        }),
    ));
    add_edge(
        graph,
        archetype_id,
        npc_id.0,
        WorldEdge::SpecificallyDependsOn,
    );
    add_edge(graph, archetype_id, npc_id.0, WorldEdge::InheresIn);

    let goal_id = graph.add_node(WorldNode::DependentContinuant(
        DependentContinuant::Disposition(DispositionNode {
            kind: DispositionKind::Goal(goal),
        }),
    ));
    add_edge(graph, goal_id, npc_id.0, WorldEdge::SpecificallyDependsOn);
    add_edge(graph, goal_id, npc_id.0, WorldEdge::InheresIn);

    for trait_tag in traits {
        let trait_id = graph.add_node(WorldNode::DependentContinuant(
            DependentContinuant::Disposition(DispositionNode {
                kind: DispositionKind::Trait(*trait_tag),
            }),
        ));
        add_edge(graph, trait_id, npc_id.0, WorldEdge::SpecificallyDependsOn);
        add_edge(graph, trait_id, npc_id.0, WorldEdge::InheresIn);
    }
}

fn random_timed_route(
    rng: &mut ChaCha8Rng,
    kind: RouteKind,
    walking: (u32, u32),
    transit: Option<(u32, u32)>,
    driving: Option<(u32, u32)>,
) -> TravelRoute {
    TravelRoute {
        kind,
        walking_time: TimeDelta::from_seconds(rng.random_range(walking.0..=walking.1)),
        transit_time: transit
            .map(|(min, max)| TimeDelta::from_seconds(rng.random_range(min..=max))),
        driving_time: driving
            .map(|(min, max)| TimeDelta::from_seconds(rng.random_range(min..=max))),
    }
}

fn mix_seed(seed: WorldSeed, parts: &[u64]) -> u64 {
    let mut value = seed.raw() ^ 0x9E37_79B9_7F4A_7C15;
    for part in parts {
        value ^= part.wrapping_add(0x9E37_79B9_7F4A_7C15);
        value = value.rotate_left(27).wrapping_mul(0x94D0_49BB_1331_11EB);
    }
    value
}

fn random_route(rng: &mut ChaCha8Rng, primary_link: bool) -> TravelRoute {
    if primary_link {
        if rng.random_bool(0.45) {
            TravelRoute {
                kind: RouteKind::ArterialRoad,
                walking_time: TimeDelta::from_seconds(rng.random_range(45 * 60..=80 * 60)),
                transit_time: Some(TimeDelta::from_seconds(rng.random_range(18 * 60..=35 * 60))),
                driving_time: Some(TimeDelta::from_seconds(rng.random_range(10 * 60..=22 * 60))),
            }
        } else {
            TravelRoute {
                kind: RouteKind::Highway,
                walking_time: TimeDelta::from_seconds(rng.random_range(2 * 60 * 60..=4 * 60 * 60)),
                transit_time: Some(TimeDelta::from_seconds(rng.random_range(45 * 60..=95 * 60))),
                driving_time: Some(TimeDelta::from_seconds(rng.random_range(30 * 60..=70 * 60))),
            }
        }
    } else if rng.random_bool(0.5) {
        TravelRoute {
            kind: RouteKind::Highway,
            walking_time: TimeDelta::from_seconds(rng.random_range(3 * 60 * 60..=6 * 60 * 60)),
            transit_time: Some(TimeDelta::from_seconds(
                rng.random_range(60 * 60..=2 * 60 * 60),
            )),
            driving_time: Some(TimeDelta::from_seconds(rng.random_range(40 * 60..=90 * 60))),
        }
    } else {
        TravelRoute {
            kind: RouteKind::ArterialRoad,
            walking_time: TimeDelta::from_seconds(rng.random_range(60 * 60..=2 * 60 * 60)),
            transit_time: Some(TimeDelta::from_seconds(rng.random_range(25 * 60..=50 * 60))),
            driving_time: Some(TimeDelta::from_seconds(rng.random_range(15 * 60..=35 * 60))),
        }
    }
}

const DISTRICT_PREFIXES: [&str; 10] = [
    "Ash", "Market", "Harbor", "Station", "North", "South", "River", "Glass", "Union", "Cedar",
];
const DISTRICT_SUFFIXES: [&str; 10] = [
    "Quarter", "Heights", "Square", "Point", "Terrace", "Center", "Row", "Reach", "Gate", "Yard",
];
const DISTRICT_TEXTURES: [&str; 8] = [
    "dense midrise blocks",
    "retail-heavy streets",
    "quiet apartment corridors",
    "office-facing avenues",
    "warehouse edges",
    "night-shift storefronts",
    "mixed-use corners",
    "narrow commuter lanes",
];
const DISTRICT_FUNCTIONS: [&str; 8] = [
    "corner stores and takeout windows",
    "small offices and service counters",
    "loading bays and fenced lots",
    "apartment entries and laundromats",
    "transit foot traffic and kiosks",
    "cafes and repair shops",
    "late-night traffic and side parking",
    "municipal buildings and walk-ups",
];
const LANDMARK_PREFIXES: [&str; 8] = [
    "Old", "North", "Glass", "Moon", "Union", "Raven", "Low", "Civic",
];
const LANDMARK_NOUNS: [&str; 8] = [
    "Exchange",
    "Museum",
    "Data Center",
    "Overpass",
    "Terminal",
    "Arcade",
    "Park",
    "Archive",
];
const CITY_PREFIXES: [&str; 16] = [
    "Ash", "Brae", "Cinder", "Dawn", "Elder", "Frost", "Glimmer", "High", "Iron", "Juniper",
    "Kings", "Low", "Moon", "North", "Quartz", "Raven",
];
const CITY_SUFFIXES: [&str; 16] = [
    "view", "ford", "grove", "crest", "point", "side", "market", "cross", "heights", "center",
    "gate", "harbor", "park", "field", "square", "junction",
];
const NPC_FIRST_NAMES: [&str; 24] = [
    "Ari", "Bryn", "Cato", "Dara", "Esme", "Finn", "Galen", "Hana", "Ivo", "Jora", "Kellan", "Lio",
    "Mara", "Niko", "Orin", "Pia", "Quin", "Rhea", "Soren", "Talia", "Una", "Vero", "Wren", "Yana",
];
const NPC_LAST_NAMES: [&str; 24] = [
    "Ashdown", "Briar", "Cask", "Dunfield", "Ember", "Farrow", "Gale", "Hearth", "Ives", "Jun",
    "Keene", "Lark", "Morrow", "Nettle", "Orchard", "Pell", "Quarry", "Reeve", "Sable", "Thorne",
    "Vale", "Wick", "Mercer", "Cross",
];
const PLACE_STREET_KINDS: [&str; 6] = [
    "main street",
    "service lane",
    "market block",
    "river block",
    "office row",
    "retail strip",
];
const PLACE_INTERIOR_KINDS: [&str; 6] = [
    "coffee shop",
    "apartment lobby",
    "coworking floor",
    "clinic entrance",
    "food hall",
    "bookstore",
];
const APARTMENT_ROOM_LABELS: [&str; 6] = ["1A", "1B", "2A", "2B", "3A", "3B"];
const VEHICLE_PREFIXES: [&str; 8] = [
    "Ashcrest",
    "Northgate",
    "Moonline",
    "Harbor",
    "Juniper",
    "Raven",
    "Quartz",
    "Lowcross",
];
const VEHICLE_MODELS: [&str; 5] = [
    "sedan",
    "hatchback",
    "delivery van",
    "compact SUV",
    "rideshare Prius",
];
const GUN_NAMES: [&str; 3] = ["compact pistol", "service revolver", "polymer handgun"];
const KNIFE_NAMES: [&str; 3] = ["pocket knife", "utility knife", "folding knife"];
const BAG_NAMES: [&str; 3] = ["duffel bag", "messenger bag", "canvas tote"];

#[cfg(test)]
mod tests {
    use super::World;
    use crate::graph_ecs::WorldEdge;
    use crate::invariants::InvariantViolation;
    use petgraph::Direction::Incoming;
    use petgraph::visit::EdgeRef;
    use riggy_ontology::seed::WorldSeed;

    #[test]
    fn procgen_is_deterministic() {
        let a = World::generate(WorldSeed::new(42), 18);
        let b = World::generate(WorldSeed::new(42), 18);
        assert_eq!(a, b);
        assert!(a.validate().is_empty());
    }

    #[test]
    fn world_is_connected_and_in_bounds() {
        let world = World::generate(WorldSeed::new(7), 24);
        assert_eq!(world.city_ids().len(), 24);

        let mut visited = std::collections::BTreeSet::new();
        let mut stack = vec![world.city_ids()[0]];
        while let Some(city_id) = stack.pop() {
            if !visited.insert(city_id) {
                continue;
            }
            stack.extend(world.city_connections(city_id));
        }

        assert_eq!(visited.len(), world.city_ids().len());
        assert!(world.npc_ids().len() >= 24 * 3);
        assert!(world.validate().is_empty());
    }

    #[test]
    fn validator_detects_missing_place_container() {
        let mut world = World::generate(WorldSeed::new(3), 16);
        let place_id = world.city_places(world.city_ids()[0])[0];
        let edge_id = world
            .graph
            .edges_directed(place_id.0, Incoming)
            .find(|edge| matches!(edge.weight(), WorldEdge::ContainsPlace))
            .map(|edge| edge.id())
            .expect("place should have containing city");
        world.graph.remove_edge(edge_id);

        assert!(
            world
                .validate()
                .contains(&InvariantViolation::PlaceMissingCity { place_id })
        );
    }

    #[test]
    fn validator_detects_npc_present_outside_resident_city() {
        let mut world = World::generate(WorldSeed::new(5), 16);
        let npc_id = world.npc_ids()[0];
        let resident_city_id = world.npc_resident_city_ids(npc_id)[0];
        let present_edge_id = world
            .graph
            .edges_directed(npc_id.0, Incoming)
            .find(|edge| matches!(edge.weight(), WorldEdge::PresentAt))
            .map(|edge| edge.id())
            .expect("npc should have current place");
        let present_city_id = world
            .city_ids()
            .into_iter()
            .find(|city_id| *city_id != resident_city_id)
            .expect("world should have another city");
        let present_place_id = world.city_places(present_city_id)[0];
        world.graph.remove_edge(present_edge_id);
        world
            .graph
            .add_edge(present_place_id.0, npc_id.0, WorldEdge::PresentAt);

        assert!(
            world
                .validate()
                .contains(&InvariantViolation::NpcPresentOutsideResidentCity {
                    npc_id,
                    resident_city_id,
                    present_city_id,
                })
        );
    }
}
