use petgraph::Direction::{Incoming, Outgoing};
use petgraph::visit::EdgeRef;
use rand::Rng;
use rand::prelude::IndexedRandom;
use rand_chacha::{ChaCha8Rng, rand_core::SeedableRng};
use serde::{Deserialize, Serialize};

use crate::domain::invariants::{InvariantViolation, validate_world};
use crate::domain::seed::WorldSeed;
use crate::domain::time::TimeDelta;
use crate::domain::vocab::{Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag};
pub use crate::graph_ecs::{CityId, EntityId, NpcId, PlaceId};
use crate::graph_ecs::{WorldEdge, WorldGraph, WorldNode, add_edge, edge_snapshot};

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransportMode {
    Walking,
    Transit,
    Car,
}

impl TransportMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Walking => "walk",
            Self::Transit => "transit",
            Self::Car => "car",
        }
    }

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RouteKind {
    Hallway,
    Stairwell,
    Crosswalk,
    SideStreet,
    LocalRoad,
    ArterialRoad,
    Highway,
}

impl RouteKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Hallway => "hallway",
            Self::Stairwell => "stairwell",
            Self::Crosswalk => "crosswalk",
            Self::SideStreet => "side street",
            Self::LocalRoad => "local roads",
            Self::ArterialRoad => "arterial road",
            Self::Highway => "highway",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct TravelRoute {
    pub kind: RouteKind,
    pub walking_seconds: u32,
    pub transit_seconds: Option<u32>,
    pub driving_seconds: Option<u32>,
}

impl TravelRoute {
    pub fn travel_time(self, mode: TransportMode) -> Option<TimeDelta> {
        match mode {
            TransportMode::Walking => Some(TimeDelta::from_seconds(self.walking_seconds)),
            TransportMode::Transit => self.transit_seconds.map(TimeDelta::from_seconds),
            TransportMode::Car => self.driving_seconds.map(TimeDelta::from_seconds),
        }
    }

    pub fn supports(self, mode: TransportMode) -> bool {
        self.travel_time(mode).is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct City {
    pub name: String,
    pub biome: Biome,
    pub economy: Economy,
    pub culture: Culture,
    pub districts: Vec<District>,
    pub landmarks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct District {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Place {
    pub district_name: String,
    pub name: String,
    pub kind: PlaceKind,
    pub description: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlaceKind {
    BuildingInterior,
    ApartmentLobby,
    ApartmentRoom,
    RoadLane,
    SidewalkLeft,
    SidewalkRight,
    StationConcourse,
    StationPlatform,
}

impl PlaceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::BuildingInterior => "building interior",
            Self::ApartmentLobby => "apartment lobby",
            Self::ApartmentRoom => "apartment room",
            Self::RoadLane => "road lane",
            Self::SidewalkLeft => "left sidewalk",
            Self::SidewalkRight => "right sidewalk",
            Self::StationConcourse => "station concourse",
            Self::StationPlatform => "station platform",
        }
    }
}

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
    pub name: String,
    pub archetype: NpcArchetype,
    pub personality_traits: Vec<TraitTag>,
    pub goal: GoalTag,
    pub occupation: Occupation,
    pub home_district: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entity {
    pub name: String,
    pub kind: EntityKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntityKind {
    Car,
    Gun,
    Knife,
    Bag,
}

impl EntityKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Car => "car",
            Self::Gun => "gun",
            Self::Knife => "knife",
            Self::Bag => "bag",
        }
    }
}

impl World {
    pub fn generate(seed: WorldSeed, city_count: usize) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed.raw());
        let target_cities = city_count.clamp(16, 24);

        let district_prefixes = [
            "North", "South", "East", "West", "Downtown", "Old", "Harbor", "River", "Station",
            "Market",
        ];
        let district_suffixes = [
            "District", "Heights", "Point", "Square", "Center", "Quarter", "Park", "Commons",
        ];
        let landmark_kinds = [
            "station",
            "museum",
            "tower",
            "mall",
            "park",
            "hospital",
            "data center",
            "stadium",
        ];
        let street_kinds = [
            "main street",
            "service lane",
            "market block",
            "river block",
            "office row",
            "retail strip",
        ];
        let building_kinds = [
            "coffee shop",
            "apartment lobby",
            "coworking floor",
            "clinic entrance",
            "food hall",
            "bookstore",
        ];
        let city_first = [
            "Ash", "Brae", "Cinder", "Dawn", "Elder", "Frost", "Glimmer", "High", "Iron",
            "Juniper", "Kings", "Low", "Moon", "North", "Oak", "Port", "Quartz", "Raven", "Stone",
            "Thorn", "Umber", "Vale", "West", "Yarrow",
        ];
        let city_second = [
            "view", "ford", "grove", "crest", "point", "side", "market", "cross", "heights",
            "center", "gate", "harbor", "park", "field", "square", "junction",
        ];
        let first_names = [
            "Ari", "Bryn", "Cato", "Dara", "Esme", "Finn", "Galen", "Hana", "Ivo", "Jora",
            "Kellan", "Lio", "Mara", "Niko", "Orin", "Pia", "Quin", "Rhea", "Soren", "Talia",
            "Una", "Vero", "Wren", "Yana",
        ];
        let last_names = [
            "Ashdown", "Briar", "Cask", "Dunfield", "Ember", "Farrow", "Gale", "Hearth", "Ives",
            "Jun", "Keene", "Lark", "Morrow", "Nettle", "Orchard", "Pell", "Quarry", "Reeve",
            "Sable", "Thorne", "Vale", "Wick",
        ];
        let mut graph = WorldGraph::default();
        let mut city_ids = Vec::with_capacity(target_cities);
        let mut city_hubs = Vec::with_capacity(target_cities);

        for ordinal in 0..target_cities {
            let name = format!(
                "{}{}",
                city_first[ordinal % city_first.len()],
                city_second[rng.random_range(0..city_second.len())]
            );
            let mut districts = Vec::new();
            let district_count = rng.random_range(3..=4);
            for _ in 0..district_count {
                let district_name = format!(
                    "{} {}",
                    district_prefixes.choose(&mut rng).unwrap(),
                    district_suffixes.choose(&mut rng).unwrap()
                );
                districts.push(District {
                    name: district_name.clone(),
                    description: format!(
                        "{} is known for its {} mood, traffic, and crowded storefronts.",
                        district_name,
                        TraitTag::ALL.choose(&mut rng).unwrap().label()
                    ),
                });
            }

            let landmark_count = rng.random_range(2..=3);
            let mut landmarks = Vec::with_capacity(landmark_count);
            for _ in 0..landmark_count {
                landmarks.push(format!(
                    "the {} {}",
                    city_first.choose(&mut rng).unwrap().to_lowercase(),
                    landmark_kinds.choose(&mut rng).unwrap()
                ));
            }

            let index = graph.add_node(WorldNode::City(City {
                name,
                biome: *Biome::ALL.choose(&mut rng).unwrap(),
                economy: *Economy::ALL.choose(&mut rng).unwrap(),
                culture: *Culture::ALL.choose(&mut rng).unwrap(),
                districts,
                landmarks,
            }));
            let city_id = CityId(index);
            city_ids.push(city_id);
        }

        for city_id in &city_ids {
            let city = Self::city_from_graph(&graph, *city_id).clone();
            let mut road_lanes = Vec::new();
            let mut pedestrian_places = Vec::new();
            for (district_index, district) in city.districts.iter().enumerate() {
                let road_id = add_place(
                    &mut graph,
                    *city_id,
                    &district.name,
                    format!(
                        "{} {}",
                        district.name,
                        street_kinds.choose(&mut rng).unwrap()
                    ),
                    PlaceKind::RoadLane,
                    format!(
                        "A vehicle lane in {} where deliveries, rideshares, and through-traffic stack up.",
                        district.name
                    ),
                );
                road_lanes.push(road_id);

                let left_sidewalk_id = add_place(
                    &mut graph,
                    *city_id,
                    &district.name,
                    format!("{} left sidewalk", district.name),
                    PlaceKind::SidewalkLeft,
                    format!(
                        "The left-side sidewalk in {} with storefront windows, signs, and steady foot traffic.",
                        district.name
                    ),
                );
                pedestrian_places.push(left_sidewalk_id);

                let right_sidewalk_id = add_place(
                    &mut graph,
                    *city_id,
                    &district.name,
                    format!("{} right sidewalk", district.name),
                    PlaceKind::SidewalkRight,
                    format!(
                        "The right-side sidewalk in {} where bus stops, benches, and curb cuts slow the flow.",
                        district.name
                    ),
                );
                pedestrian_places.push(right_sidewalk_id);

                let curb_route = TravelRoute {
                    kind: RouteKind::Crosswalk,
                    walking_seconds: rng.random_range(8..=20),
                    transit_seconds: None,
                    driving_seconds: None,
                };
                add_edge(
                    &mut graph,
                    road_id.0,
                    left_sidewalk_id.0,
                    WorldEdge::TravelRoute(curb_route),
                );
                add_edge(
                    &mut graph,
                    left_sidewalk_id.0,
                    road_id.0,
                    WorldEdge::TravelRoute(curb_route),
                );
                add_edge(
                    &mut graph,
                    road_id.0,
                    right_sidewalk_id.0,
                    WorldEdge::TravelRoute(curb_route),
                );
                add_edge(
                    &mut graph,
                    right_sidewalk_id.0,
                    road_id.0,
                    WorldEdge::TravelRoute(curb_route),
                );

                let sidewalk_crossing = TravelRoute {
                    kind: RouteKind::Crosswalk,
                    walking_seconds: rng.random_range(15..=35),
                    transit_seconds: None,
                    driving_seconds: None,
                };
                add_edge(
                    &mut graph,
                    left_sidewalk_id.0,
                    right_sidewalk_id.0,
                    WorldEdge::TravelRoute(sidewalk_crossing),
                );
                add_edge(
                    &mut graph,
                    right_sidewalk_id.0,
                    left_sidewalk_id.0,
                    WorldEdge::TravelRoute(sidewalk_crossing),
                );

                let building_id = add_place(
                    &mut graph,
                    *city_id,
                    &district.name,
                    format!(
                        "{} {}",
                        district.name,
                        building_kinds.choose(&mut rng).unwrap()
                    ),
                    PlaceKind::BuildingInterior,
                    format!(
                        "An interior space in {} where people slow down, talk longer, and watch who comes through.",
                        district.name
                    ),
                );
                pedestrian_places.push(building_id);

                add_edge(
                    &mut graph,
                    left_sidewalk_id.0,
                    building_id.0,
                    WorldEdge::TravelRoute(TravelRoute {
                        kind: RouteKind::Hallway,
                        walking_seconds: rng.random_range(8..=20),
                        transit_seconds: None,
                        driving_seconds: None,
                    }),
                );
                add_edge(
                    &mut graph,
                    building_id.0,
                    left_sidewalk_id.0,
                    WorldEdge::TravelRoute(TravelRoute {
                        kind: RouteKind::Hallway,
                        walking_seconds: rng.random_range(8..=20),
                        transit_seconds: None,
                        driving_seconds: None,
                    }),
                );

                if city_id.index() == 0 && district_index == 0 {
                    let lobby_id = add_place(
                        &mut graph,
                        *city_id,
                        &district.name,
                        format!("{} Apartments Lobby", district.name),
                        PlaceKind::ApartmentLobby,
                        format!(
                            "A modest apartment lobby in {} with mailboxes, a buzzer panel, and scuffed tile from years of foot traffic.",
                            district.name
                        ),
                    );
                    pedestrian_places.push(lobby_id);

                    let hall_route = TravelRoute {
                        kind: RouteKind::Hallway,
                        walking_seconds: rng.random_range(6..=14),
                        transit_seconds: None,
                        driving_seconds: None,
                    };
                    add_edge(
                        &mut graph,
                        left_sidewalk_id.0,
                        lobby_id.0,
                        WorldEdge::TravelRoute(hall_route),
                    );
                    add_edge(
                        &mut graph,
                        lobby_id.0,
                        left_sidewalk_id.0,
                        WorldEdge::TravelRoute(hall_route),
                    );

                    for room_number in ["1A", "1B", "2A", "2B"] {
                        let room_id = add_place(
                            &mut graph,
                            *city_id,
                            &district.name,
                            format!("{} Apartments {}", district.name, room_number),
                            PlaceKind::ApartmentRoom,
                            format!(
                                "A small apartment unit in {} with a narrow kitchen, thin walls, and just enough space to disappear for a while.",
                                district.name
                            ),
                        );
                        pedestrian_places.push(room_id);

                        let room_route = TravelRoute {
                            kind: RouteKind::Hallway,
                            walking_seconds: rng.random_range(4..=12),
                            transit_seconds: None,
                            driving_seconds: None,
                        };
                        add_edge(
                            &mut graph,
                            lobby_id.0,
                            room_id.0,
                            WorldEdge::TravelRoute(room_route),
                        );
                        add_edge(
                            &mut graph,
                            room_id.0,
                            lobby_id.0,
                            WorldEdge::TravelRoute(room_route),
                        );
                    }
                }

                if district_index == 0 || rng.random_bool(0.55) {
                    let entity_id = add_entity(
                        &mut graph,
                        format!("{} {}", district.name, car_model_name(&mut rng)),
                        EntityKind::Car,
                    );
                    add_edge(
                        &mut graph,
                        road_id.0,
                        entity_id.0,
                        WorldEdge::ContainsEntity,
                    );
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
                    let entity_id = add_entity(
                        &mut graph,
                        format!(
                            "{} {}",
                            district.name,
                            loose_item_name(&mut rng, entity_kind)
                        ),
                        entity_kind,
                    );
                    add_edge(
                        &mut graph,
                        sidewalk_target.0,
                        entity_id.0,
                        WorldEdge::ContainsEntity,
                    );
                }
            }

            let hub_district = city
                .districts
                .first()
                .map(|district| district.name.clone())
                .unwrap_or_else(|| city.name.clone());
            let concourse_id = add_place(
                &mut graph,
                *city_id,
                &hub_district,
                format!("{} Central Concourse", city.name),
                PlaceKind::StationConcourse,
                "A loud indoor concourse full of departure boards, kiosks, and hurried transfers."
                    .to_string(),
            );
            pedestrian_places.push(concourse_id);

            let platform_id = add_place(
                &mut graph,
                *city_id,
                &hub_district,
                format!("{} Platform Level", city.name),
                PlaceKind::StationPlatform,
                "Open-air platforms and curbside bays where regional departures actually leave."
                    .to_string(),
            );
            pedestrian_places.push(platform_id);
            city_hubs.push(platform_id);

            let station_link = TravelRoute {
                kind: RouteKind::Stairwell,
                walking_seconds: rng.random_range(18..=45),
                transit_seconds: None,
                driving_seconds: None,
            };
            add_edge(
                &mut graph,
                concourse_id.0,
                platform_id.0,
                WorldEdge::TravelRoute(station_link),
            );
            add_edge(
                &mut graph,
                platform_id.0,
                concourse_id.0,
                WorldEdge::TravelRoute(station_link),
            );

            for window in road_lanes.windows(2) {
                let route = TravelRoute {
                    kind: RouteKind::LocalRoad,
                    walking_seconds: rng.random_range(60..=180),
                    transit_seconds: None,
                    driving_seconds: Some(rng.random_range(20..=60)),
                };
                add_edge(
                    &mut graph,
                    window[0].0,
                    window[1].0,
                    WorldEdge::TravelRoute(route),
                );
                add_edge(
                    &mut graph,
                    window[1].0,
                    window[0].0,
                    WorldEdge::TravelRoute(route),
                );
            }
            if road_lanes.len() > 2 {
                let a = road_lanes[0];
                let b = *road_lanes.last().unwrap();
                let route = TravelRoute {
                    kind: RouteKind::LocalRoad,
                    walking_seconds: rng.random_range(120..=360),
                    transit_seconds: Some(rng.random_range(60..=180)),
                    driving_seconds: Some(rng.random_range(30..=120)),
                };
                add_edge(&mut graph, a.0, b.0, WorldEdge::TravelRoute(route));
                add_edge(&mut graph, b.0, a.0, WorldEdge::TravelRoute(route));
            }

            for window in pedestrian_places.windows(2) {
                let route = TravelRoute {
                    kind: RouteKind::SideStreet,
                    walking_seconds: rng.random_range(20..=90),
                    transit_seconds: None,
                    driving_seconds: None,
                };
                add_edge(
                    &mut graph,
                    window[0].0,
                    window[1].0,
                    WorldEdge::TravelRoute(route),
                );
                add_edge(
                    &mut graph,
                    window[1].0,
                    window[0].0,
                    WorldEdge::TravelRoute(route),
                );
            }

            if let Some(station_sidewalk) = pedestrian_places.first().copied() {
                let station_access = TravelRoute {
                    kind: RouteKind::Hallway,
                    walking_seconds: rng.random_range(20..=60),
                    transit_seconds: None,
                    driving_seconds: None,
                };
                add_edge(
                    &mut graph,
                    station_sidewalk.0,
                    concourse_id.0,
                    WorldEdge::TravelRoute(station_access),
                );
                add_edge(
                    &mut graph,
                    concourse_id.0,
                    station_sidewalk.0,
                    WorldEdge::TravelRoute(station_access),
                );
            }
        }

        for pos in 0..target_cities {
            let next = (pos + 1) % target_cities;
            let route = random_route(&mut rng, true);
            add_edge(
                &mut graph,
                city_ids[pos].0,
                city_ids[next].0,
                WorldEdge::TravelRoute(route),
            );
            add_edge(
                &mut graph,
                city_ids[next].0,
                city_ids[pos].0,
                WorldEdge::TravelRoute(route),
            );
            add_edge(
                &mut graph,
                city_hubs[pos].0,
                city_hubs[next].0,
                WorldEdge::TravelRoute(route),
            );
            add_edge(
                &mut graph,
                city_hubs[next].0,
                city_hubs[pos].0,
                WorldEdge::TravelRoute(route),
            );
        }

        for _ in 0..(target_cities / 2) {
            let a = rng.random_range(0..target_cities);
            let mut b = rng.random_range(0..target_cities);
            while b == a {
                b = rng.random_range(0..target_cities);
            }
            let route = random_route(&mut rng, false);
            add_edge(
                &mut graph,
                city_ids[a].0,
                city_ids[b].0,
                WorldEdge::TravelRoute(route),
            );
            add_edge(
                &mut graph,
                city_ids[b].0,
                city_ids[a].0,
                WorldEdge::TravelRoute(route),
            );
            add_edge(
                &mut graph,
                city_hubs[a].0,
                city_hubs[b].0,
                WorldEdge::TravelRoute(route),
            );
            add_edge(
                &mut graph,
                city_hubs[b].0,
                city_hubs[a].0,
                WorldEdge::TravelRoute(route),
            );
        }

        for city_id in &city_ids {
            let district_names = Self::city_from_graph(&graph, *city_id)
                .districts
                .iter()
                .map(|district| district.name.clone())
                .collect::<Vec<_>>();
            let mut possible_places = Self::city_places_from_graph(&graph, *city_id)
                .into_iter()
                .filter(|place_id| {
                    Self::place_from_graph(&graph, *place_id)
                        .kind
                        .supports_people()
                })
                .collect::<Vec<_>>();
            if city_id.index() == 0 {
                if let Some(lobby_id) = possible_places.iter().copied().find(|place_id| {
                    matches!(
                        Self::place_from_graph(&graph, *place_id).kind,
                        PlaceKind::ApartmentLobby
                    )
                }) {
                    possible_places.retain(|place_id| *place_id != lobby_id);
                    possible_places.insert(0, lobby_id);
                }
            }
            let npc_count = rng.random_range(3..=5);
            for npc_offset in 0..npc_count {
                let name = format!(
                    "{} {}",
                    first_names.choose(&mut rng).unwrap(),
                    last_names.choose(&mut rng).unwrap()
                );
                let mut personality_traits = TraitTag::ALL
                    .choose_multiple(&mut rng, 2)
                    .copied()
                    .collect::<Vec<_>>();
                personality_traits.sort();
                let index = graph.add_node(WorldNode::Npc(Npc {
                    name,
                    archetype: *NpcArchetype::ALL.choose(&mut rng).unwrap(),
                    personality_traits,
                    goal: *GoalTag::ALL.choose(&mut rng).unwrap(),
                    occupation: *Occupation::ALL.choose(&mut rng).unwrap(),
                    home_district: district_names.choose(&mut rng).unwrap().clone(),
                }));
                let npc_id = NpcId(index);
                add_edge(&mut graph, city_id.0, npc_id.0, WorldEdge::Resident);
                if let Some(place_id) = possible_places
                    .get(npc_offset % possible_places.len())
                    .copied()
                {
                    add_edge(&mut graph, place_id.0, npc_id.0, WorldEdge::PresentAt);
                }
            }
        }

        Self { seed, graph }
    }

    pub fn city(&self, id: CityId) -> &City {
        Self::city_from_graph(&self.graph, id)
    }

    pub fn npc(&self, id: NpcId) -> &Npc {
        match self.graph.node_weight(id.0) {
            Some(WorldNode::Npc(npc)) => npc,
            _ => panic!("invalid npc id {:?}", id),
        }
    }

    pub fn place(&self, id: PlaceId) -> &Place {
        Self::place_from_graph(&self.graph, id)
    }

    pub fn entity(&self, id: EntityId) -> &Entity {
        match self.graph.node_weight(id.0) {
            Some(WorldNode::Entity(entity)) => entity,
            _ => panic!("invalid entity id {:?}", id),
        }
    }

    pub fn validate(&self) -> Vec<InvariantViolation> {
        validate_world(self)
    }

    pub fn city_ids(&self) -> Vec<CityId> {
        self.graph
            .node_indices()
            .filter_map(|index| match self.graph.node_weight(index) {
                Some(WorldNode::City(_)) => Some(CityId(index)),
                _ => None,
            })
            .collect()
    }

    pub fn npc_ids(&self) -> Vec<NpcId> {
        self.graph
            .node_indices()
            .filter_map(|index| match self.graph.node_weight(index) {
                Some(WorldNode::Npc(_)) => Some(NpcId(index)),
                _ => None,
            })
            .collect()
    }

    pub fn city_connections(&self, city_id: CityId) -> Vec<CityId> {
        self.graph
            .edges_directed(city_id.0, Outgoing)
            .filter(|edge| matches!(edge.weight(), WorldEdge::TravelRoute(_)))
            .map(|edge| CityId(edge.target()))
            .collect()
    }

    pub fn city_npcs(&self, city_id: CityId) -> Vec<NpcId> {
        Self::city_npcs_from_graph(&self.graph, city_id)
    }

    pub fn city_places(&self, city_id: CityId) -> Vec<PlaceId> {
        Self::city_places_from_graph(&self.graph, city_id)
    }

    pub fn place_routes(&self, place_id: PlaceId) -> Vec<(PlaceId, TravelRoute)> {
        self.graph
            .edges_directed(place_id.0, Outgoing)
            .filter_map(|edge| match edge.weight() {
                WorldEdge::TravelRoute(route) => match self.graph.node_weight(edge.target()) {
                    Some(WorldNode::Place(_)) => Some((PlaceId(edge.target()), *route)),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    pub fn place_npcs(&self, place_id: PlaceId) -> Vec<NpcId> {
        self.graph
            .edges_directed(place_id.0, Outgoing)
            .filter(|edge| matches!(edge.weight(), WorldEdge::PresentAt))
            .map(|edge| NpcId(edge.target()))
            .collect()
    }

    pub fn place_entities(&self, place_id: PlaceId) -> Vec<EntityId> {
        self.graph
            .edges_directed(place_id.0, Outgoing)
            .filter(|edge| matches!(edge.weight(), WorldEdge::ContainsEntity))
            .map(|edge| EntityId(edge.target()))
            .collect()
    }

    pub fn place_cars(&self, place_id: PlaceId) -> Vec<EntityId> {
        self.place_entities(place_id)
            .into_iter()
            .filter(|entity_id| matches!(self.entity(*entity_id).kind, EntityKind::Car))
            .collect()
    }

    pub fn entity_place_id(&self, entity_id: EntityId) -> Option<PlaceId> {
        self.graph
            .edges_directed(entity_id.0, Incoming)
            .find(|edge| matches!(edge.weight(), WorldEdge::ContainsEntity))
            .map(|edge| PlaceId(edge.source()))
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
        self.graph
            .edges_directed(place_id.0, Incoming)
            .find(|edge| matches!(edge.weight(), WorldEdge::ContainsPlace))
            .map(|edge| CityId(edge.source()))
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
        graph
            .edges_directed(city_id.0, Outgoing)
            .filter(|edge| matches!(edge.weight(), WorldEdge::Resident))
            .map(|edge| NpcId(edge.target()))
            .collect()
    }

    fn city_places_from_graph(graph: &WorldGraph, city_id: CityId) -> Vec<PlaceId> {
        graph
            .edges_directed(city_id.0, Outgoing)
            .filter(|edge| matches!(edge.weight(), WorldEdge::ContainsPlace))
            .map(|edge| PlaceId(edge.target()))
            .collect()
    }
}

fn add_place(
    graph: &mut WorldGraph,
    city_id: CityId,
    district_name: &str,
    name: String,
    kind: PlaceKind,
    description: String,
) -> PlaceId {
    let index = graph.add_node(WorldNode::Place(Place {
        district_name: district_name.to_string(),
        name,
        kind,
        description,
    }));
    let place_id = PlaceId(index);
    add_edge(graph, city_id.0, place_id.0, WorldEdge::ContainsPlace);
    place_id
}

fn add_entity(graph: &mut WorldGraph, name: String, kind: EntityKind) -> EntityId {
    let index = graph.add_node(WorldNode::Entity(Entity { name, kind }));
    EntityId(index)
}

fn car_model_name(rng: &mut ChaCha8Rng) -> &'static str {
    const MODELS: &[&str] = &[
        "sedan",
        "hatchback",
        "delivery van",
        "compact SUV",
        "rideshare Prius",
    ];
    MODELS.choose(rng).copied().unwrap_or("sedan")
}

fn loose_item_name(rng: &mut ChaCha8Rng, kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Car => car_model_name(rng),
        EntityKind::Gun => {
            const NAMES: &[&str] = &["compact pistol", "service revolver", "polymer handgun"];
            NAMES.choose(rng).copied().unwrap_or("compact pistol")
        }
        EntityKind::Knife => {
            const NAMES: &[&str] = &["pocket knife", "utility knife", "folding knife"];
            NAMES.choose(rng).copied().unwrap_or("pocket knife")
        }
        EntityKind::Bag => {
            const NAMES: &[&str] = &["duffel bag", "messenger bag", "canvas tote"];
            NAMES.choose(rng).copied().unwrap_or("duffel bag")
        }
    }
}

fn random_route(rng: &mut ChaCha8Rng, primary_link: bool) -> TravelRoute {
    if primary_link {
        if rng.random_bool(0.45) {
            TravelRoute {
                kind: RouteKind::ArterialRoad,
                walking_seconds: rng.random_range(45 * 60..=80 * 60),
                transit_seconds: Some(rng.random_range(18 * 60..=35 * 60)),
                driving_seconds: Some(rng.random_range(10 * 60..=22 * 60)),
            }
        } else {
            TravelRoute {
                kind: RouteKind::Highway,
                walking_seconds: rng.random_range(2 * 60 * 60..=4 * 60 * 60),
                transit_seconds: Some(rng.random_range(45 * 60..=95 * 60)),
                driving_seconds: Some(rng.random_range(30 * 60..=70 * 60)),
            }
        }
    } else if rng.random_bool(0.5) {
        TravelRoute {
            kind: RouteKind::Highway,
            walking_seconds: rng.random_range(3 * 60 * 60..=6 * 60 * 60),
            transit_seconds: Some(rng.random_range(60 * 60..=2 * 60 * 60)),
            driving_seconds: Some(rng.random_range(40 * 60..=90 * 60)),
        }
    } else {
        TravelRoute {
            kind: RouteKind::ArterialRoad,
            walking_seconds: rng.random_range(60 * 60..=2 * 60 * 60),
            transit_seconds: Some(rng.random_range(25 * 60..=50 * 60)),
            driving_seconds: Some(rng.random_range(15 * 60..=35 * 60)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::World;
    use crate::domain::invariants::InvariantViolation;
    use crate::graph_ecs::WorldEdge;
    use petgraph::Direction::Incoming;
    use petgraph::visit::EdgeRef;

    #[test]
    fn procgen_is_deterministic() {
        let a = World::generate(crate::domain::seed::WorldSeed::new(42), 18);
        let b = World::generate(crate::domain::seed::WorldSeed::new(42), 18);
        assert_eq!(a, b);
        assert!(a.validate().is_empty());
    }

    #[test]
    fn world_is_connected_and_in_bounds() {
        let world = World::generate(crate::domain::seed::WorldSeed::new(7), 24);
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
        let mut world = World::generate(crate::domain::seed::WorldSeed::new(3), 16);
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
        let mut world = World::generate(crate::domain::seed::WorldSeed::new(5), 16);
        let npc_id = world.npc_ids()[0];
        let resident_city_id = world
            .graph
            .edges_directed(npc_id.0, Incoming)
            .find(|edge| matches!(edge.weight(), WorldEdge::Resident))
            .map(|edge| crate::graph_ecs::CityId(edge.source()))
            .expect("npc should have resident city");
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
