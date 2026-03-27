use petgraph::Directed;
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use rand::Rng;
use rand::prelude::IndexedRandom;
use rand_chacha::{ChaCha8Rng, rand_core::SeedableRng};
use serde::{Deserialize, Serialize};

use crate::domain::memory::ConversationMemory;
use crate::domain::records::{ContextEntry, DialogueLine};
use crate::domain::seed::WorldSeed;
use crate::domain::time::{GameTime, TimeDelta};
use crate::domain::vocab::{Biome, Culture, Economy, GoalTag, NpcArchetype, Occupation, TraitTag};

macro_rules! node_id_type {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
        )]
        pub struct $name(pub NodeIndex);

        impl $name {
            pub fn index(self) -> usize {
                self.0.index()
            }
        }
    };
}

node_id_type!(CountryId);
node_id_type!(CityId);
node_id_type!(PlaceId);
node_id_type!(ActorId);
node_id_type!(EntityId);
node_id_type!(ProcessId);
node_id_type!(RecordId);
node_id_type!(ClockId);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NodeId {
    Country(CountryId),
    City(CityId),
    Place(PlaceId),
    Actor(ActorId),
    Item(EntityId),
    Process(ProcessId),
    Record(RecordId),
    Clock(ClockId),
}

impl NodeId {
    pub fn index(self) -> usize {
        match self {
            Self::Country(id) => id.index(),
            Self::City(id) => id.index(),
            Self::Place(id) => id.index(),
            Self::Actor(id) => id.index(),
            Self::Item(id) => id.index(),
            Self::Process(id) => id.index(),
            Self::Record(id) => id.index(),
            Self::Clock(id) => id.index(),
        }
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

impl ActorId {
    pub fn name(self, seed: WorldSeed) -> String {
        let combination_count = ACTOR_FIRST_NAMES.len() * ACTOR_LAST_NAMES.len();
        let offset = (mix_seed(seed, &[1, 0]) as usize) % combination_count;
        let stride = actor_name_stride(seed, combination_count);
        let slot = (offset + self.index().saturating_mul(stride)) % combination_count;
        let first_index = slot % ACTOR_FIRST_NAMES.len();
        let last_index = slot / ACTOR_FIRST_NAMES.len();
        let base = format!(
            "{} {}",
            ACTOR_FIRST_NAMES[first_index], ACTOR_LAST_NAMES[last_index]
        );
        let cycle = self.index() / combination_count;
        if cycle == 0 {
            base
        } else {
            format!("{base} {}", cycle + 1)
        }
    }
}

macro_rules! labeled_enum {
    ($name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

labeled_enum!(RouteKind {
    Walk => "walking route",
    Transit => "transit line",
});

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TravelRoute {
    pub kind: RouteKind,
    pub travel_time: TimeDelta,
}

labeled_enum!(PlaceKind {
    Room => "room",
});

impl PlaceKind {
    pub const fn supports_people(self) -> bool {
        true
    }
}

labeled_enum!(EntityKind {
    Gun => "gun",
    Knife => "knife",
    Bag => "bag",
});

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControllerMode {
    Manual,
    AiAgent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Country;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct City {
    pub biome: Biome,
    pub economy: Economy,
    pub culture: Culture,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Place {
    pub kind: PlaceKind,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Actor {
    pub controller: ControllerMode,
    pub occupation: Occupation,
    pub archetype: NpcArchetype,
    pub traits: Vec<TraitTag>,
    pub goal: GoalTag,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entity {
    pub kind: EntityKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Process {
    pub kind: ProcessKind,
    pub started_at: GameTime,
    pub duration: TimeDelta,
    pub ended_at: GameTime,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcessKind {
    Speak,
    Travel,
    Waiting,
    Inspect,
    DoNothing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Record {
    ConversationMemory(ConversationMemory),
    Dialogue(DialogueLine),
    Context(ContextEntry),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Clock {
    pub current_time: GameTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorProfile {
    pub controller: ControllerMode,
    pub occupation: Occupation,
    pub archetype: NpcArchetype,
    pub traits: Vec<TraitTag>,
    pub goal: GoalTag,
    pub home_place_id: PlaceId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorldNode {
    Country(Country),
    City(City),
    Place(Place),
    Actor(Actor),
    Item(Entity),
    Process(Process),
    Record(Record),
    Clock(Clock),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorldRelation {
    Contains,
    Connected(TravelRoute),
    LocatedAt,
    Home,
    InInventoryOf,
    Participates,
    OccursAt,
    Targets,
    HasTranscript,
    HasContext,
    HasMemory,
    KnowsCity { discovered_at: GameTime },
}

pub type WorldGraph = StableGraph<WorldNode, WorldRelation, Directed>;

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

impl World {
    pub fn generate(seed: WorldSeed, world_size_hint: usize) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed.raw());
        let room_count = world_size_hint.clamp(4, 6);
        let mut world = Self {
            seed,
            graph: WorldGraph::default(),
        };

        let country_id = world.add_country();
        world.add_clock(GameTime::from_seconds(0));

        let city_id = world.add_city(City {
            biome: *Biome::ALL.choose(&mut rng).unwrap(),
            economy: *Economy::ALL.choose(&mut rng).unwrap(),
            culture: *Culture::ALL.choose(&mut rng).unwrap(),
        });
        add_edge(
            &mut world.graph,
            country_id.0,
            city_id.0,
            WorldRelation::Contains,
        );
        let start_place_id = world.build_building_rooms(&mut rng, city_id, room_count);
        world.spawn_room_ai_actors(&mut rng, city_id, 3);

        world.add_actor(
            ControllerMode::Manual,
            Occupation::SoftwareEngineer,
            NpcArchetype::Watcher,
            vec![TraitTag::Patient, TraitTag::Guarded],
            GoalTag::VerifyOnlineRumor,
            start_place_id,
            start_place_id,
        );

        world
    }

    pub fn node_id(&self, index: NodeIndex) -> Option<NodeId> {
        match self.graph.node_weight(index)? {
            WorldNode::Country(_) => Some(NodeId::Country(CountryId(index))),
            WorldNode::City(_) => Some(NodeId::City(CityId(index))),
            WorldNode::Place(_) => Some(NodeId::Place(PlaceId(index))),
            WorldNode::Actor(_) => Some(NodeId::Actor(ActorId(index))),
            WorldNode::Item(_) => Some(NodeId::Item(EntityId(index))),
            WorldNode::Process(_) => Some(NodeId::Process(ProcessId(index))),
            WorldNode::Record(_) => Some(NodeId::Record(RecordId(index))),
            WorldNode::Clock(_) => Some(NodeId::Clock(ClockId(index))),
        }
    }

    pub fn city(&self, id: CityId) -> &City {
        match self.graph.node_weight(id.0) {
            Some(WorldNode::City(city)) => city,
            _ => panic!("city id {:?} is invalid", id),
        }
    }

    pub fn city_name(&self, id: CityId) -> String {
        id.name(self.seed)
    }

    pub fn actor(&self, id: ActorId) -> &Actor {
        match self.graph.node_weight(id.0) {
            Some(WorldNode::Actor(actor)) => actor,
            _ => panic!("actor id {:?} is invalid", id),
        }
    }

    pub fn actor_profile(&self, id: ActorId) -> ActorProfile {
        let actor = self.actor(id);
        ActorProfile {
            controller: actor.controller,
            occupation: actor.occupation,
            archetype: actor.archetype,
            traits: actor.traits.clone(),
            goal: actor.goal,
            home_place_id: self
                .actor_home_place_id(id)
                .expect("actor should have a home place"),
        }
    }

    pub fn actor_name(&self, id: ActorId) -> String {
        id.name(self.seed)
    }

    pub fn place(&self, id: PlaceId) -> &Place {
        match self.graph.node_weight(id.0) {
            Some(WorldNode::Place(place)) => place,
            _ => panic!("place id {:?} is invalid", id),
        }
    }

    pub fn place_name(&self, id: PlaceId) -> String {
        let city_id = self
            .place_city_id(id)
            .expect("place should belong to a city for naming");
        place_name_from_parts(self.seed, id, city_id, self.place(id).kind)
    }

    pub fn entity(&self, id: EntityId) -> &Entity {
        match self.graph.node_weight(id.0) {
            Some(WorldNode::Item(entity)) => entity,
            _ => panic!("entity id {:?} is invalid", id),
        }
    }

    pub fn entity_name(&self, id: EntityId) -> String {
        entity_name_from_parts(self.seed, id, self.entity(id).kind)
    }

    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();

        let country_ids = self.collect_node_ids(|index, node| match node {
            WorldNode::Country(_) => Some(CountryId(index)),
            _ => None,
        });
        if country_ids.len() != 1 {
            issues.push(format!(
                "expected exactly one country node, found {}",
                country_ids.len()
            ));
        }

        let clock_ids = self.collect_node_ids(|index, node| match node {
            WorldNode::Clock(_) => Some(ClockId(index)),
            _ => None,
        });
        if clock_ids.len() != 1 {
            issues.push(format!(
                "expected exactly one clock node, found {}",
                clock_ids.len()
            ));
        }

        let manual_actor_ids = self
            .actor_ids()
            .into_iter()
            .filter(|actor_id| self.actor(*actor_id).controller == ControllerMode::Manual)
            .collect::<Vec<_>>();
        if manual_actor_ids.len() != 1 {
            issues.push(format!(
                "expected exactly one manual actor, found {}",
                manual_actor_ids.len()
            ));
        }

        for city_id in self.city_ids() {
            let incoming_country_count = self
                .graph
                .edges_directed(city_id.0, Incoming)
                .filter(|edge| {
                    matches!(edge.weight(), WorldRelation::Contains)
                        && matches!(
                            self.graph.node_weight(edge.source()),
                            Some(WorldNode::Country(_))
                        )
                })
                .count();
            if incoming_country_count != 1 {
                issues.push(format!(
                    "city {} should belong to exactly one country, found {}",
                    city_id.index(),
                    incoming_country_count
                ));
            }

            for connected in self.city_connections(city_id) {
                if !self.has_city(connected) {
                    issues.push(format!(
                        "city {} references missing connected city {}",
                        city_id.index(),
                        connected.index()
                    ));
                    continue;
                }
                if !self.city_connections(connected).contains(&city_id) {
                    issues.push(format!(
                        "city connection {} -> {} is not symmetric",
                        city_id.index(),
                        connected.index()
                    ));
                }
            }

            if self.city_places(city_id).is_empty() {
                issues.push(format!("city {} has no contained places", city_id.index()));
            }
        }

        for place_id in self.collect_node_ids(|index, node| match node {
            WorldNode::Place(_) => Some(PlaceId(index)),
            _ => None,
        }) {
            let containing_cities = self.collect_incoming(place_id.0, |source, node, relation| {
                if matches!(relation, WorldRelation::Contains) && matches!(node, WorldNode::City(_))
                {
                    Some(CityId(source))
                } else {
                    None
                }
            });
            if containing_cities.len() != 1 {
                issues.push(format!(
                    "place {} should belong to exactly one city, found {}",
                    place_id.index(),
                    containing_cities.len()
                ));
            }

            for (destination, route) in self.place_routes(place_id) {
                if !self.has_place(destination) {
                    issues.push(format!(
                        "place {} references missing route destination {}",
                        place_id.index(),
                        destination.index()
                    ));
                    continue;
                }
                if !self.place_routes(destination).iter().any(
                    |(reverse_destination, reverse_route)| {
                        *reverse_destination == place_id && *reverse_route == route
                    },
                ) {
                    issues.push(format!(
                        "route {} -> {} is not symmetric",
                        place_id.index(),
                        destination.index()
                    ));
                }
            }
        }

        for actor_id in self.actor_ids() {
            let home_places = self.collect_outgoing(actor_id.0, |target, node, relation| {
                if matches!(relation, WorldRelation::Home) && matches!(node, WorldNode::Place(_)) {
                    Some(PlaceId(target))
                } else {
                    None
                }
            });
            if home_places.len() != 1 {
                issues.push(format!(
                    "actor {} should have exactly one home place, found {}",
                    actor_id.index(),
                    home_places.len()
                ));
            }

            let present_places = self.collect_outgoing(actor_id.0, |target, node, relation| {
                if matches!(relation, WorldRelation::LocatedAt)
                    && matches!(node, WorldNode::Place(_))
                {
                    Some(PlaceId(target))
                } else {
                    None
                }
            });
            if present_places.len() != 1 {
                issues.push(format!(
                    "actor {} should have exactly one present place, found {}",
                    actor_id.index(),
                    present_places.len()
                ));
            }

            if let (Some(home_place), Some(present_place)) = (
                home_places.first().copied(),
                present_places.first().copied(),
            ) {
                let home_city = self.place_city_id(home_place);
                let present_city = self.place_city_id(present_place);
                if home_city != present_city {
                    issues.push(format!(
                        "actor {} lives in city {:?} but is present in {:?}",
                        actor_id.index(),
                        home_city.map(CityId::index),
                        present_city.map(CityId::index)
                    ));
                }
            }

            let memory_count = self
                .graph
                .edges_directed(actor_id.0, Outgoing)
                .filter(|edge| {
                    matches!(edge.weight(), WorldRelation::HasMemory)
                        && matches!(
                            self.graph.node_weight(edge.target()),
                            Some(WorldNode::Record(Record::ConversationMemory(_)))
                        )
                })
                .count();
            if memory_count != 1 {
                issues.push(format!(
                    "actor {} should have exactly one memory record, found {}",
                    actor_id.index(),
                    memory_count
                ));
            }
        }

        for entity_id in self.collect_node_ids(|index, node| match node {
            WorldNode::Item(_) => Some(EntityId(index)),
            _ => None,
        }) {
            let location_edges = self
                .graph
                .edges_directed(entity_id.0, Outgoing)
                .filter(|edge| {
                    matches!(
                        edge.weight(),
                        WorldRelation::LocatedAt | WorldRelation::InInventoryOf
                    )
                })
                .count();
            if location_edges != 1 {
                issues.push(format!(
                    "entity {} should have exactly one location/inventory relation, found {}",
                    entity_id.index(),
                    location_edges
                ));
            }
        }

        for process_id in self.collect_node_ids(|index, node| match node {
            WorldNode::Process(_) => Some(ProcessId(index)),
            _ => None,
        }) {
            match self.process(process_id).kind {
                ProcessKind::Speak => {
                    let participants = self.process_actor_participants(process_id);
                    if participants.len() != 2 {
                        issues.push(format!(
                            "speak process {} should have exactly two actor participants, found {}",
                            process_id.index(),
                            participants.len()
                        ));
                    }
                    if self.process_place(process_id).is_none() {
                        issues.push(format!(
                            "speak process {} should occur at exactly one place",
                            process_id.index()
                        ));
                    }
                    if self.process_dialogue_lines(process_id).is_empty() {
                        issues.push(format!(
                            "speak process {} should include transcript lines",
                            process_id.index()
                        ));
                    }
                }
                ProcessKind::Travel => {
                    if self.process_actor_participants(process_id).len() != 1 {
                        issues.push(format!(
                            "travel process {} should have exactly one actor participant",
                            process_id.index()
                        ));
                    }
                    if self.process_target_place(process_id).is_none() {
                        issues.push(format!(
                            "travel process {} should target exactly one destination",
                            process_id.index()
                        ));
                    }
                }
                ProcessKind::Waiting => {
                    if self.process_actor_participants(process_id).len() != 1 {
                        issues.push(format!(
                            "waiting process {} should have exactly one actor participant",
                            process_id.index()
                        ));
                    }
                    if self.process_place(process_id).is_none() {
                        issues.push(format!(
                            "waiting process {} should occur at exactly one place",
                            process_id.index()
                        ));
                    }
                }
                ProcessKind::Inspect => {
                    if self.process_actor_participants(process_id).len() != 1 {
                        issues.push(format!(
                            "inspect process {} should have exactly one actor participant",
                            process_id.index()
                        ));
                    }
                    if self.process_place(process_id).is_none() {
                        issues.push(format!(
                            "inspect process {} should occur at exactly one place",
                            process_id.index()
                        ));
                    }
                    if self.process_target_entity(process_id).is_none() {
                        issues.push(format!(
                            "inspect process {} should target exactly one entity",
                            process_id.index()
                        ));
                    }
                }
                ProcessKind::DoNothing => {
                    if self.process_actor_participants(process_id).len() != 1 {
                        issues.push(format!(
                            "do_nothing process {} should have exactly one actor participant",
                            process_id.index()
                        ));
                    }
                    if self.process_place(process_id).is_none() {
                        issues.push(format!(
                            "do_nothing process {} should occur at exactly one place",
                            process_id.index()
                        ));
                    }
                }
            }
        }

        for record_id in self.collect_node_ids(|index, node| match node {
            WorldNode::Record(_) => Some(RecordId(index)),
            _ => None,
        }) {
            match self.record(record_id) {
                Record::ConversationMemory(_) => {
                    let incoming = self
                        .graph
                        .edges_directed(record_id.0, Incoming)
                        .filter(|edge| matches!(edge.weight(), WorldRelation::HasMemory))
                        .count();
                    if incoming != 1 {
                        issues.push(format!(
                            "memory record {} should belong to exactly one actor, found {}",
                            record_id.index(),
                            incoming
                        ));
                    }
                }
                Record::Dialogue(_) => {
                    let transcript_links = self
                        .graph
                        .edges_directed(record_id.0, Incoming)
                        .filter(|edge| matches!(edge.weight(), WorldRelation::HasTranscript))
                        .count();
                    if transcript_links != 1 {
                        issues.push(format!(
                            "dialogue record {} should belong to exactly one process transcript, found {}",
                            record_id.index(),
                            transcript_links
                        ));
                    }
                }
                Record::Context(_) => {
                    let context_links = self
                        .graph
                        .edges_directed(record_id.0, Incoming)
                        .filter(|edge| matches!(edge.weight(), WorldRelation::HasContext))
                        .count();
                    if context_links == 0 {
                        issues.push(format!(
                            "context record {} should belong to at least one actor context feed",
                            record_id.index()
                        ));
                    }
                }
            }
        }

        for edge in self.graph.edge_references() {
            if let WorldRelation::Connected(route) = edge.weight() {
                let source = self.node_id(edge.source());
                let target = self.node_id(edge.target());
                let valid = matches!(
                    (source, target),
                    (Some(NodeId::City(_)), Some(NodeId::City(_)))
                        | (Some(NodeId::Place(_)), Some(NodeId::Place(_)))
                );
                if !valid {
                    issues.push(format!(
                        "connected relation {:?} -> {:?} with {:?} must link cities or places of the same kind",
                        source,
                        target,
                        route
                    ));
                }
            }
        }

        issues
    }

    pub fn current_time(&self) -> GameTime {
        let clock_id = self.clock_id().expect("world should contain a clock");
        match self.graph.node_weight(clock_id.0) {
            Some(WorldNode::Clock(clock)) => clock.current_time,
            _ => panic!("clock node should be valid"),
        }
    }

    pub fn set_current_time(&mut self, current_time: GameTime) {
        let clock_id = self.clock_id().expect("world should contain a clock");
        let Some(WorldNode::Clock(clock)) = self.graph.node_weight_mut(clock_id.0) else {
            panic!("clock node should be valid");
        };
        clock.current_time = current_time;
    }

    pub fn actor_ids(&self) -> Vec<ActorId> {
        self.collect_node_ids(|index, node| match node {
            WorldNode::Actor(_) => Some(ActorId(index)),
            _ => None,
        })
    }

    pub fn manual_actor_id(&self) -> Option<ActorId> {
        self.actor_ids()
            .into_iter()
            .find(|actor_id| self.actor(*actor_id).controller == ControllerMode::Manual)
    }

    pub fn actor_place_id(&self, actor_id: ActorId) -> Option<PlaceId> {
        if !self.has_actor(actor_id) {
            return None;
        }
        self.collect_outgoing(actor_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::LocatedAt) && matches!(node, WorldNode::Place(_)) {
                Some(PlaceId(target))
            } else {
                None
            }
        })
        .into_iter()
        .next()
    }

    pub fn actor_city_id(&self, actor_id: ActorId) -> Option<CityId> {
        self.actor_place_id(actor_id)
            .and_then(|place_id| self.place_city_id(place_id))
    }

    pub fn move_actor(&mut self, actor_id: ActorId, place_id: PlaceId) {
        if !self.has_actor(actor_id) || !self.has_place(place_id) {
            return;
        }
        self.replace_outgoing_relation(
            actor_id.0,
            WorldRelationMatcher::LocatedAt,
            place_id.0,
            WorldRelation::LocatedAt,
        );
    }

    pub fn set_actor_home(&mut self, actor_id: ActorId, place_id: PlaceId) {
        if !self.has_actor(actor_id) || !self.has_place(place_id) {
            return;
        }
        self.replace_outgoing_relation(
            actor_id.0,
            WorldRelationMatcher::Home,
            place_id.0,
            WorldRelation::Home,
        );
    }

    pub fn city_ids(&self) -> Vec<CityId> {
        self.collect_node_ids(|index, node| match node {
            WorldNode::City(_) => Some(CityId(index)),
            _ => None,
        })
    }

    pub fn city_connections(&self, city_id: CityId) -> Vec<CityId> {
        let mut ids = self.collect_outgoing(city_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::Connected(_)) && matches!(node, WorldNode::City(_))
            {
                Some(CityId(target))
            } else {
                None
            }
        });
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub fn city_places(&self, city_id: CityId) -> Vec<PlaceId> {
        let mut ids = self.collect_outgoing(city_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::Contains) && matches!(node, WorldNode::Place(_)) {
                Some(PlaceId(target))
            } else {
                None
            }
        });
        ids.sort_unstable();
        ids
    }

    pub fn city_actors(&self, city_id: CityId) -> Vec<ActorId> {
        self.actor_ids()
            .into_iter()
            .filter(|actor_id| self.actor_resident_city_ids(*actor_id).contains(&city_id))
            .collect()
    }

    pub fn place_routes(&self, place_id: PlaceId) -> Vec<(PlaceId, TravelRoute)> {
        let mut routes = self.collect_outgoing(place_id.0, |target, node, relation| {
            if let WorldRelation::Connected(route) = relation {
                if matches!(node, WorldNode::Place(_)) {
                    return Some((PlaceId(target), *route));
                }
            }
            None
        });
        routes.sort_unstable_by_key(|(destination, _)| destination.index());
        routes
    }

    pub fn place_actors(&self, place_id: PlaceId) -> Vec<ActorId> {
        let mut ids = self.collect_incoming(place_id.0, |source, node, relation| {
            if matches!(relation, WorldRelation::LocatedAt) && matches!(node, WorldNode::Actor(_)) {
                Some(ActorId(source))
            } else {
                None
            }
        });
        ids.sort_unstable();
        ids
    }

    pub fn place_entities(&self, place_id: PlaceId) -> Vec<EntityId> {
        let mut ids = self.collect_incoming(place_id.0, |source, node, relation| {
            if matches!(relation, WorldRelation::LocatedAt) && matches!(node, WorldNode::Item(_)) {
                Some(EntityId(source))
            } else {
                None
            }
        });
        ids.sort_unstable();
        ids
    }

    pub fn actor_resident_city_ids(&self, actor_id: ActorId) -> Vec<CityId> {
        self.actor_home_place_id(actor_id)
            .and_then(|place_id| self.place_city_id(place_id))
            .into_iter()
            .collect()
    }

    pub fn actor_present_place_ids(&self, actor_id: ActorId) -> Vec<PlaceId> {
        self.collect_outgoing(actor_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::LocatedAt) && matches!(node, WorldNode::Place(_)) {
                Some(PlaceId(target))
            } else {
                None
            }
        })
    }

    pub fn actor_conversation_memory(&self, actor_id: ActorId) -> Option<ConversationMemory> {
        let record_id = self
            .collect_outgoing(actor_id.0, |target, node, relation| {
                if matches!(relation, WorldRelation::HasMemory)
                    && matches!(node, WorldNode::Record(Record::ConversationMemory(_)))
                {
                    Some(RecordId(target))
                } else {
                    None
                }
            })
            .into_iter()
            .next()?;
        match self.record(record_id) {
            Record::ConversationMemory(memory) if !memory.is_empty() => Some(memory.clone()),
            _ => None,
        }
    }

    pub fn merge_actor_conversation_memory(
        &mut self,
        actor_id: ActorId,
        update: ConversationMemory,
    ) {
        if !self.has_actor(actor_id) {
            return;
        }
        let update = update.normalized();
        if update.is_empty() {
            return;
        }

        if let Some(record_id) = self
            .collect_outgoing(actor_id.0, |target, node, relation| {
                if matches!(relation, WorldRelation::HasMemory)
                    && matches!(node, WorldNode::Record(Record::ConversationMemory(_)))
                {
                    Some(RecordId(target))
                } else {
                    None
                }
            })
            .into_iter()
            .next()
        {
            let Some(WorldNode::Record(Record::ConversationMemory(memory))) =
                self.graph.node_weight_mut(record_id.0)
            else {
                return;
            };
            memory.merge_update(update);
            return;
        }

        let record_id = RecordId(
            self.graph
                .add_node(WorldNode::Record(Record::ConversationMemory(update))),
        );
        add_edge(
            &mut self.graph,
            actor_id.0,
            record_id.0,
            WorldRelation::HasMemory,
        );
    }

    pub fn discovered_city_ids(&self, actor_id: ActorId) -> Vec<CityId> {
        if !self.has_actor(actor_id) {
            return Vec::new();
        }

        let mut ids = self.collect_outgoing(actor_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::KnowsCity { .. })
                && matches!(node, WorldNode::City(_))
            {
                Some(CityId(target))
            } else {
                None
            }
        });
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub fn discover_city(&mut self, actor_id: ActorId, city_id: CityId, discovered_at: GameTime) {
        if !self.has_actor(actor_id) || !self.has_city(city_id) {
            return;
        }
        if self
            .graph
            .edges_connecting(actor_id.0, city_id.0)
            .any(|edge| matches!(edge.weight(), WorldRelation::KnowsCity { .. }))
        {
            return;
        }
        add_edge(
            &mut self.graph,
            actor_id.0,
            city_id.0,
            WorldRelation::KnowsCity { discovered_at },
        );
    }

    pub fn speech_lines_between(
        &self,
        actor_id: ActorId,
        counterpart_id: ActorId,
        limit: usize,
    ) -> Vec<DialogueLine> {
        let mut lines = self
            .collect_node_ids(|index, node| match node {
                WorldNode::Process(process) if process.kind == ProcessKind::Speak => {
                    Some(ProcessId(index))
                }
                _ => None,
            })
            .into_iter()
            .filter(|process_id| {
                let participants = self.process_actor_participants(*process_id);
                participants.len() == 2
                    && participants.contains(&actor_id)
                    && participants.contains(&counterpart_id)
            })
            .flat_map(|process_id| self.process_dialogue_lines(process_id))
            .collect::<Vec<_>>();
        lines.sort_by_key(|line| line.timestamp);
        let len = lines.len();
        lines.into_iter().skip(len.saturating_sub(limit)).collect()
    }

    pub fn record_speech_process(
        &mut self,
        actor_id: ActorId,
        counterpart_id: ActorId,
        place_id: PlaceId,
        started_at: GameTime,
        duration: TimeDelta,
        transcript: Vec<DialogueLine>,
    ) -> ProcessId {
        let process_id = self.add_process(ProcessKind::Speak, started_at, duration);
        add_edge(
            &mut self.graph,
            process_id.0,
            actor_id.0,
            WorldRelation::Participates,
        );
        add_edge(
            &mut self.graph,
            process_id.0,
            counterpart_id.0,
            WorldRelation::Participates,
        );
        add_edge(
            &mut self.graph,
            process_id.0,
            place_id.0,
            WorldRelation::OccursAt,
        );
        for line in transcript {
            self.append_process_dialogue_line(process_id, &[actor_id, counterpart_id], line);
        }
        process_id
    }

    pub fn record_travel_process(
        &mut self,
        actor_id: ActorId,
        destination_id: PlaceId,
        started_at: GameTime,
        duration: TimeDelta,
    ) -> ProcessId {
        let process_id = self.add_process(ProcessKind::Travel, started_at, duration);
        add_edge(
            &mut self.graph,
            process_id.0,
            actor_id.0,
            WorldRelation::Participates,
        );
        add_edge(
            &mut self.graph,
            process_id.0,
            destination_id.0,
            WorldRelation::Targets,
        );
        process_id
    }

    pub fn record_waiting_process(
        &mut self,
        actor_id: ActorId,
        place_id: PlaceId,
        started_at: GameTime,
        duration: TimeDelta,
    ) -> ProcessId {
        let process_id = self.add_process(ProcessKind::Waiting, started_at, duration);
        add_edge(
            &mut self.graph,
            process_id.0,
            actor_id.0,
            WorldRelation::Participates,
        );
        add_edge(
            &mut self.graph,
            process_id.0,
            place_id.0,
            WorldRelation::OccursAt,
        );
        process_id
    }

    pub fn record_inspect_process(
        &mut self,
        actor_id: ActorId,
        entity_id: EntityId,
        place_id: PlaceId,
        started_at: GameTime,
        duration: TimeDelta,
    ) -> ProcessId {
        let process_id = self.add_process(ProcessKind::Inspect, started_at, duration);
        add_edge(
            &mut self.graph,
            process_id.0,
            actor_id.0,
            WorldRelation::Participates,
        );
        add_edge(
            &mut self.graph,
            process_id.0,
            entity_id.0,
            WorldRelation::Targets,
        );
        add_edge(
            &mut self.graph,
            process_id.0,
            place_id.0,
            WorldRelation::OccursAt,
        );
        process_id
    }

    pub fn record_do_nothing_process(
        &mut self,
        actor_id: ActorId,
        place_id: PlaceId,
        started_at: GameTime,
    ) -> ProcessId {
        let process_id = self.add_process(ProcessKind::DoNothing, started_at, TimeDelta::ZERO);
        add_edge(
            &mut self.graph,
            process_id.0,
            actor_id.0,
            WorldRelation::Participates,
        );
        add_edge(
            &mut self.graph,
            process_id.0,
            place_id.0,
            WorldRelation::OccursAt,
        );
        process_id
    }

    pub fn append_context_entry(&mut self, actor_id: ActorId, entry: ContextEntry) {
        if !self.has_actor(actor_id) {
            return;
        }
        let record_id = RecordId(
            self.graph
                .add_node(WorldNode::Record(Record::Context(entry))),
        );
        add_edge(
            &mut self.graph,
            actor_id.0,
            record_id.0,
            WorldRelation::HasContext,
        );
    }

    pub fn recent_context_entries(&self, actor_id: ActorId, limit: usize) -> Vec<ContextEntry> {
        if !self.has_actor(actor_id) {
            return Vec::new();
        }

        let mut entries = self.collect_outgoing(actor_id.0, |_target, node, relation| {
            if !matches!(relation, WorldRelation::HasContext) {
                return None;
            }
            match node {
                WorldNode::Record(Record::Dialogue(line)) => {
                    Some((line.timestamp, ContextEntry::Dialogue(line.clone())))
                }
                WorldNode::Record(Record::Context(entry)) => {
                    Some((context_timestamp(entry), entry.clone()))
                }
                _ => None,
            }
        });
        entries.sort_by_key(|(timestamp, entry)| (*timestamp, context_sort_rank(entry)));
        let len = entries.len();
        entries
            .into_iter()
            .skip(len.saturating_sub(limit))
            .map(|(_, entry)| entry)
            .collect()
    }

    pub fn place_city_id(&self, place_id: PlaceId) -> Option<CityId> {
        self.collect_incoming(place_id.0, |source, node, relation| {
            if matches!(relation, WorldRelation::Contains) && matches!(node, WorldNode::City(_)) {
                Some(CityId(source))
            } else {
                None
            }
        })
        .into_iter()
        .next()
    }

    fn process(&self, process_id: ProcessId) -> &Process {
        match self.graph.node_weight(process_id.0) {
            Some(WorldNode::Process(process)) => process,
            _ => panic!("process id {:?} is invalid", process_id),
        }
    }

    fn process_actor_participants(&self, process_id: ProcessId) -> Vec<ActorId> {
        let mut ids = self.collect_outgoing(process_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::Participates)
                && matches!(node, WorldNode::Actor(_))
            {
                Some(ActorId(target))
            } else {
                None
            }
        });
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    fn process_place(&self, process_id: ProcessId) -> Option<PlaceId> {
        self.collect_outgoing(process_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::OccursAt) && matches!(node, WorldNode::Place(_)) {
                Some(PlaceId(target))
            } else {
                None
            }
        })
        .into_iter()
        .next()
    }

    fn process_target_place(&self, process_id: ProcessId) -> Option<PlaceId> {
        self.collect_outgoing(process_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::Targets) && matches!(node, WorldNode::Place(_)) {
                Some(PlaceId(target))
            } else {
                None
            }
        })
        .into_iter()
        .next()
    }

    fn process_target_entity(&self, process_id: ProcessId) -> Option<EntityId> {
        self.collect_outgoing(process_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::Targets) && matches!(node, WorldNode::Item(_)) {
                Some(EntityId(target))
            } else {
                None
            }
        })
        .into_iter()
        .next()
    }

    fn process_dialogue_lines(&self, process_id: ProcessId) -> Vec<DialogueLine> {
        let mut transcript = self.collect_outgoing(process_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::HasTranscript)
                && matches!(node, WorldNode::Record(Record::Dialogue(_)))
            {
                let Some(WorldNode::Record(Record::Dialogue(line))) =
                    self.graph.node_weight(target)
                else {
                    return None;
                };
                Some(line.clone())
            } else {
                None
            }
        });
        transcript.sort_by_key(|line| line.timestamp);
        transcript
    }

    fn record(&self, record_id: RecordId) -> &Record {
        match self.graph.node_weight(record_id.0) {
            Some(WorldNode::Record(record)) => record,
            _ => panic!("record id {:?} is invalid", record_id),
        }
    }

    fn add_country(&mut self) -> CountryId {
        CountryId(self.graph.add_node(WorldNode::Country(Country)))
    }

    fn add_clock(&mut self, current_time: GameTime) -> ClockId {
        ClockId(
            self.graph
                .add_node(WorldNode::Clock(Clock { current_time })),
        )
    }

    fn add_city(&mut self, city: City) -> CityId {
        CityId(self.graph.add_node(WorldNode::City(city)))
    }

    fn add_place(&mut self, city_id: CityId, kind: PlaceKind, description: String) -> PlaceId {
        let place_id = PlaceId(
            self.graph
                .add_node(WorldNode::Place(Place { kind, description })),
        );
        add_edge(
            &mut self.graph,
            city_id.0,
            place_id.0,
            WorldRelation::Contains,
        );
        place_id
    }

    fn add_actor(
        &mut self,
        controller: ControllerMode,
        occupation: Occupation,
        archetype: NpcArchetype,
        mut traits: Vec<TraitTag>,
        goal: GoalTag,
        home_place_id: PlaceId,
        current_place_id: PlaceId,
    ) -> ActorId {
        traits.sort();
        traits.dedup();
        let actor_id = ActorId(self.graph.add_node(WorldNode::Actor(Actor {
            controller,
            occupation,
            archetype,
            traits,
            goal,
        })));
        add_edge(
            &mut self.graph,
            actor_id.0,
            home_place_id.0,
            WorldRelation::Home,
        );
        add_edge(
            &mut self.graph,
            actor_id.0,
            current_place_id.0,
            WorldRelation::LocatedAt,
        );
        let memory_id = RecordId(self.graph.add_node(WorldNode::Record(
            Record::ConversationMemory(ConversationMemory::default()),
        )));
        add_edge(
            &mut self.graph,
            actor_id.0,
            memory_id.0,
            WorldRelation::HasMemory,
        );
        actor_id
    }

    fn add_entity_to_place(&mut self, place_id: PlaceId, kind: EntityKind) -> EntityId {
        let entity_id = EntityId(self.graph.add_node(WorldNode::Item(Entity { kind })));
        add_edge(
            &mut self.graph,
            entity_id.0,
            place_id.0,
            WorldRelation::LocatedAt,
        );
        entity_id
    }

    fn add_process(
        &mut self,
        kind: ProcessKind,
        started_at: GameTime,
        duration: TimeDelta,
    ) -> ProcessId {
        ProcessId(self.graph.add_node(WorldNode::Process(Process {
            kind,
            started_at,
            duration,
            ended_at: started_at.advance(duration),
        })))
    }

    fn append_process_dialogue_line(
        &mut self,
        process_id: ProcessId,
        context_actors: &[ActorId],
        line: DialogueLine,
    ) {
        let record_id = RecordId(
            self.graph
                .add_node(WorldNode::Record(Record::Dialogue(line.clone()))),
        );
        add_edge(
            &mut self.graph,
            process_id.0,
            record_id.0,
            WorldRelation::HasTranscript,
        );
        for actor_id in context_actors {
            add_edge(
                &mut self.graph,
                actor_id.0,
                record_id.0,
                WorldRelation::HasContext,
            );
        }
    }

    fn build_building_rooms(
        &mut self,
        rng: &mut ChaCha8Rng,
        city_id: CityId,
        room_count: usize,
    ) -> PlaceId {
        let city_name = city_id.name(self.seed);
        let mut room_ids = Vec::with_capacity(room_count);

        for room_index in 0..room_count {
            let room_id = self.add_place(
                city_id,
                PlaceKind::Room,
                format!(
                    "Room {} inside a cramped building in {} where every conversation leaks under a door somewhere.",
                    room_index + 1,
                    city_name,
                ),
            );
            room_ids.push(room_id);
        }

        for window in room_ids.windows(2) {
            self.connect_places(window[0], window[1], random_walk_route(rng, (4, 12)));
        }
        if room_ids.len() >= 4 {
            self.connect_places(
                room_ids[0],
                room_ids[room_ids.len() - 1],
                random_walk_route(rng, (8, 18)),
            );
        }
        if room_ids.len() >= 5 {
            self.connect_places(room_ids[1], room_ids[3], random_walk_route(rng, (3, 10)));
        }

        for room_id in room_ids.iter().copied() {
            if rng.random_bool(0.35) {
                let entity_kind = if rng.random_bool(0.15) {
                    EntityKind::Gun
                } else if rng.random_bool(0.45) {
                    EntityKind::Knife
                } else {
                    EntityKind::Bag
                };
                self.add_entity_to_place(room_id, entity_kind);
            }
        }

        room_ids[0]
    }

    fn connect_places(&mut self, a: PlaceId, b: PlaceId, route: TravelRoute) {
        add_edge(&mut self.graph, a.0, b.0, WorldRelation::Connected(route));
        add_edge(&mut self.graph, b.0, a.0, WorldRelation::Connected(route));
    }

    fn spawn_room_ai_actors(
        &mut self,
        rng: &mut ChaCha8Rng,
        city_id: CityId,
        actors_per_room: usize,
    ) {
        let possible_places = self
            .city_places(city_id)
            .into_iter()
            .filter(|place_id| self.place(*place_id).kind.supports_people())
            .collect::<Vec<_>>();

        for place_id in possible_places {
            for _ in 0..actors_per_room {
                let mut traits = TraitTag::ALL
                    .choose_multiple(rng, 2)
                    .copied()
                    .collect::<Vec<_>>();
                traits.sort();
                self.add_actor(
                    ControllerMode::AiAgent,
                    *Occupation::ALL.choose(rng).unwrap(),
                    *NpcArchetype::ALL.choose(rng).unwrap(),
                    traits,
                    *GoalTag::ALL.choose(rng).unwrap(),
                    place_id,
                    place_id,
                );
            }
        }
    }

    fn actor_home_place_id(&self, actor_id: ActorId) -> Option<PlaceId> {
        self.collect_outgoing(actor_id.0, |target, node, relation| {
            if matches!(relation, WorldRelation::Home) && matches!(node, WorldNode::Place(_)) {
                Some(PlaceId(target))
            } else {
                None
            }
        })
        .into_iter()
        .next()
    }

    fn clock_id(&self) -> Option<ClockId> {
        self.collect_node_ids(|index, node| match node {
            WorldNode::Clock(_) => Some(ClockId(index)),
            _ => None,
        })
        .into_iter()
        .next()
    }

    fn replace_outgoing_relation(
        &mut self,
        source: NodeIndex,
        matcher: WorldRelationMatcher,
        target: NodeIndex,
        relation: WorldRelation,
    ) {
        let to_remove = self
            .graph
            .edges_directed(source, Outgoing)
            .filter(|edge| matcher.matches(edge.weight()))
            .map(|edge| edge.id())
            .collect::<Vec<_>>();
        for edge_id in to_remove {
            self.graph.remove_edge(edge_id);
        }
        add_edge(&mut self.graph, source, target, relation);
    }

    fn has_city(&self, city_id: CityId) -> bool {
        matches!(self.graph.node_weight(city_id.0), Some(WorldNode::City(_)))
    }

    fn has_place(&self, place_id: PlaceId) -> bool {
        matches!(
            self.graph.node_weight(place_id.0),
            Some(WorldNode::Place(_))
        )
    }

    fn has_actor(&self, actor_id: ActorId) -> bool {
        matches!(
            self.graph.node_weight(actor_id.0),
            Some(WorldNode::Actor(_))
        )
    }

    fn collect_node_ids<T>(&self, map: impl Fn(NodeIndex, &WorldNode) -> Option<T>) -> Vec<T> {
        self.graph
            .node_indices()
            .filter_map(|index| {
                self.graph
                    .node_weight(index)
                    .and_then(|node| map(index, node))
            })
            .collect()
    }

    fn collect_outgoing<T>(
        &self,
        source: NodeIndex,
        map: impl Fn(NodeIndex, &WorldNode, &WorldRelation) -> Option<T>,
    ) -> Vec<T> {
        self.graph
            .edges_directed(source, Outgoing)
            .filter_map(|edge| {
                self.graph
                    .node_weight(edge.target())
                    .and_then(|node| map(edge.target(), node, edge.weight()))
            })
            .collect()
    }

    fn collect_incoming<T>(
        &self,
        target: NodeIndex,
        map: impl Fn(NodeIndex, &WorldNode, &WorldRelation) -> Option<T>,
    ) -> Vec<T> {
        self.graph
            .edges_directed(target, Incoming)
            .filter_map(|edge| {
                self.graph
                    .node_weight(edge.source())
                    .and_then(|node| map(edge.source(), node, edge.weight()))
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
enum WorldRelationMatcher {
    LocatedAt,
    Home,
}

impl WorldRelationMatcher {
    fn matches(self, relation: &WorldRelation) -> bool {
        match self {
            Self::LocatedAt => matches!(relation, WorldRelation::LocatedAt),
            Self::Home => matches!(relation, WorldRelation::Home),
        }
    }
}

fn add_edge(graph: &mut WorldGraph, source: NodeIndex, target: NodeIndex, relation: WorldRelation) {
    let exists = graph
        .edges_connecting(source, target)
        .any(|edge| edge.weight() == &relation);
    if !exists {
        graph.add_edge(source, target, relation);
    }
}

fn edge_snapshot(graph: &WorldGraph) -> Vec<(usize, usize, WorldRelation)> {
    let mut edges = graph
        .edge_references()
        .map(|edge| {
            (
                edge.source().index(),
                edge.target().index(),
                edge.weight().clone(),
            )
        })
        .collect::<Vec<_>>();
    edges.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    edges
}

fn context_timestamp(entry: &ContextEntry) -> GameTime {
    match entry {
        ContextEntry::System { timestamp, .. } => *timestamp,
        ContextEntry::Dialogue(line) => line.timestamp,
    }
}

fn context_sort_rank(entry: &ContextEntry) -> u8 {
    match entry {
        ContextEntry::System { .. } => 0,
        ContextEntry::Dialogue(_) => 1,
    }
}

fn actor_name_stride(seed: WorldSeed, combination_count: usize) -> usize {
    let mut stride = ((mix_seed(seed, &[1, 1]) as usize) % combination_count).max(1);
    while gcd_usize(stride, combination_count) != 1 {
        stride += 1;
        if stride >= combination_count {
            stride = 1;
        }
    }
    stride
}

fn room_name_slot(seed: WorldSeed, id: PlaceId) -> usize {
    let count = ROOM_NAMES.len();
    let offset = (mix_seed(seed, &[2, 0]) as usize) % count;
    let mut stride = ((mix_seed(seed, &[2, 1]) as usize) % count).max(1);
    while gcd_usize(stride, count) != 1 {
        stride += 1;
        if stride >= count {
            stride = 1;
        }
    }
    (offset + id.index().saturating_mul(stride)) % count
}

fn gcd_usize(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

pub fn place_name_from_parts(
    seed: WorldSeed,
    id: PlaceId,
    city_id: CityId,
    kind: PlaceKind,
) -> String {
    let city_name = city_id.name(seed);
    match kind {
        PlaceKind::Room => format!("{} {}", city_name, ROOM_NAMES[room_name_slot(seed, id)]),
    }
}

pub fn entity_name_from_parts(seed: WorldSeed, id: EntityId, kind: EntityKind) -> String {
    match kind {
        EntityKind::Gun => GUN_NAMES
            [(mix_seed(seed, &[5, id.index() as u64]) as usize) % GUN_NAMES.len()]
        .to_string(),
        EntityKind::Knife => KNIFE_NAMES
            [(mix_seed(seed, &[6, id.index() as u64]) as usize) % KNIFE_NAMES.len()]
        .to_string(),
        EntityKind::Bag => BAG_NAMES
            [(mix_seed(seed, &[7, id.index() as u64]) as usize) % BAG_NAMES.len()]
        .to_string(),
    }
}

fn random_walk_route(rng: &mut ChaCha8Rng, seconds: (u32, u32)) -> TravelRoute {
    TravelRoute {
        kind: RouteKind::Walk,
        travel_time: TimeDelta::from_seconds(rng.random_range(seconds.0..=seconds.1)),
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

const CITY_PREFIXES: [&str; 16] = [
    "Ash", "Brae", "Cinder", "Dawn", "Elder", "Frost", "Glimmer", "High", "Iron", "Juniper",
    "Kings", "Low", "Moon", "North", "Quartz", "Raven",
];
const CITY_SUFFIXES: [&str; 16] = [
    "view", "ford", "grove", "crest", "point", "side", "market", "cross", "heights", "center",
    "gate", "harbor", "park", "field", "square", "junction",
];
const ACTOR_FIRST_NAMES: [&str; 24] = [
    "Ari", "Bryn", "Cato", "Dara", "Esme", "Finn", "Galen", "Hana", "Ivo", "Jora", "Kellan", "Lio",
    "Mara", "Niko", "Orin", "Pia", "Quin", "Rhea", "Soren", "Talia", "Una", "Vero", "Wren", "Yana",
];
const ACTOR_LAST_NAMES: [&str; 24] = [
    "Ashdown", "Briar", "Cask", "Dunfield", "Ember", "Farrow", "Gale", "Hearth", "Ives", "Jun",
    "Keene", "Lark", "Morrow", "Nettle", "Orchard", "Pell", "Quarry", "Reeve", "Sable", "Thorne",
    "Vale", "Wick", "Mercer", "Cross",
];
const ROOM_NAMES: [&str; 8] = [
    "Lobby",
    "Common Room",
    "Kitchen",
    "Laundry Room",
    "Boiler Room",
    "Hallway",
    "Storage Room",
    "Office",
];
const GUN_NAMES: [&str; 3] = ["compact pistol", "service revolver", "polymer handgun"];
const KNIFE_NAMES: [&str; 3] = ["pocket knife", "utility knife", "folding knife"];
const BAG_NAMES: [&str; 3] = ["duffel bag", "messenger bag", "canvas tote"];

#[cfg(test)]
mod tests {
    use petgraph::Direction::Outgoing;
    use petgraph::visit::{EdgeRef, IntoEdgeReferences};

    use super::{ActorId, PlaceId, ProcessKind, World, WorldNode, WorldRelation};
    use crate::domain::events::DialogueSpeaker;
    use crate::domain::records::DialogueLine;
    use crate::domain::seed::WorldSeed;
    use crate::domain::time::{GameTime, TimeDelta};

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
        assert_eq!(world.city_ids().len(), 1);

        let city_id = world.city_ids()[0];
        let places = world.city_places(city_id);
        let mut visited = std::collections::BTreeSet::new();
        let mut stack = vec![places[0]];
        while let Some(place_id) = stack.pop() {
            if !visited.insert(place_id) {
                continue;
            }
            stack.extend(
                world
                    .place_routes(place_id)
                    .into_iter()
                    .map(|(destination, _)| destination),
            );
        }

        assert_eq!(visited.len(), places.len());
        assert!((4..=6).contains(&places.len()));
        assert_eq!(world.actor_ids().len(), places.len() * 3 + 1);
        assert!(world.validate().is_empty());
        assert!(world.manual_actor_id().is_some());
    }

    #[test]
    fn actor_location_is_represented_by_relation() {
        let mut world = World::generate(WorldSeed::new(42), 16);
        let actor_id = world.manual_actor_id().unwrap();
        let destination = world.city_places(world.city_ids()[0])[0];

        world.move_actor(actor_id, destination);

        assert_eq!(world.actor_place_id(actor_id), Some(destination));
        let outgoing = world
            .graph
            .edges_directed(actor_id.0, Outgoing)
            .filter(|edge| matches!(edge.weight(), WorldRelation::LocatedAt))
            .count();
        assert_eq!(outgoing, 1);
        assert!(matches!(
            world.graph.node_weight(destination.0),
            Some(WorldNode::Place(_))
        ));
    }

    #[test]
    fn speech_processes_are_timed_graph_nodes() {
        let mut world = World::generate(WorldSeed::new(11), 16);
        let manual_actor_id = world.manual_actor_id().unwrap();
        let place_id = world.actor_place_id(manual_actor_id).unwrap();
        let target_id = world
            .place_actors(place_id)
            .into_iter()
            .find(|actor_id| *actor_id != manual_actor_id)
            .unwrap_or_else(|| {
                let actor_id = world
                    .actor_ids()
                    .into_iter()
                    .find(|actor_id| *actor_id != manual_actor_id)
                    .unwrap();
                world.move_actor(actor_id, place_id);
                actor_id
            });
        let process_id = world.record_speech_process(
            manual_actor_id,
            target_id,
            place_id,
            GameTime::from_seconds(10),
            TimeDelta::from_seconds(20),
            vec![
                DialogueLine {
                    timestamp: GameTime::from_seconds(10),
                    speaker: DialogueSpeaker::Actor(manual_actor_id),
                    text: "hello".to_string(),
                },
                DialogueLine {
                    timestamp: GameTime::from_seconds(20),
                    speaker: DialogueSpeaker::Actor(target_id),
                    text: "hi".to_string(),
                },
            ],
        );

        assert_eq!(world.process(process_id).kind, ProcessKind::Speak);
        assert_eq!(
            world
                .speech_lines_between(manual_actor_id, target_id, 8)
                .len(),
            2
        );
        assert!(world.validate().is_empty());
    }

    #[test]
    fn item_locations_are_graph_edges() {
        let world = World::generate(WorldSeed::new(11), 16);
        let maybe_item_place = world.graph.edge_references().find_map(|edge| {
            match (
                world.graph.node_weight(edge.source()),
                world.graph.node_weight(edge.target()),
                edge.weight(),
            ) {
                (Some(WorldNode::Item(_)), Some(WorldNode::Place(_)), WorldRelation::LocatedAt) => {
                    Some(PlaceId(edge.target()))
                }
                _ => None,
            }
        });

        if let Some(place_id) = maybe_item_place {
            assert!(!world.place_entities(place_id).is_empty());
        }
    }

    #[test]
    fn exactly_one_manual_actor_exists() {
        let world = World::generate(WorldSeed::new(13), 16);
        let manual_actor_ids = world
            .actor_ids()
            .into_iter()
            .filter(|actor_id| world.actor(*actor_id).controller == super::ControllerMode::Manual)
            .collect::<Vec<ActorId>>();
        assert_eq!(manual_actor_ids.len(), 1);
    }

    #[test]
    fn actor_names_are_unique_for_generated_world_scale() {
        let world = World::generate(WorldSeed::new(42), 18);
        let names = world
            .actor_ids()
            .into_iter()
            .map(|actor_id| actor_id.name(world.seed))
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(names.len(), world.actor_ids().len());
    }

    #[test]
    fn room_names_are_unique_within_the_building() {
        let world = World::generate(WorldSeed::new(42), 18);
        let city_id = world.city_ids()[0];
        let names = world
            .city_places(city_id)
            .into_iter()
            .map(|place_id| world.place_name(place_id))
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(names.len(), world.city_places(city_id).len());
    }
}
