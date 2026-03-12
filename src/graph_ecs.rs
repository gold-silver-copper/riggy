use petgraph::Directed;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use serde::{Deserialize, Serialize};

use crate::world::{
    City, DependentContinuant, Entity, InformationContent, Npc, Occurrent, Place, Player,
    TemporalRegion, TravelRoute,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CityId(pub(crate) NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NpcId(pub(crate) NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaceId(pub(crate) NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(pub(crate) NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub(crate) NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub(crate) NodeIndex);

impl CityId {
    pub fn index(self) -> usize {
        self.0.index()
    }
}

impl NpcId {
    pub fn index(self) -> usize {
        self.0.index()
    }
}

impl PlaceId {
    pub fn index(self) -> usize {
        self.0.index()
    }
}

impl EntityId {
    pub fn index(self) -> usize {
        self.0.index()
    }
}

impl PlayerId {
    pub fn index(self) -> usize {
        self.0.index()
    }
}

impl ProcessId {
    pub fn index(self) -> usize {
        self.0.index()
    }
}

pub type WorldGraph = StableGraph<WorldNode, WorldEdge, Directed>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BfoClass {
    Entity,
    Continuant,
    IndependentContinuant,
    MaterialEntity,
    Object,
    ImmaterialEntity,
    Site,
    SpecificallyDependentContinuant,
    Role,
    Disposition,
    Function,
    Quality,
    GenericallyDependentContinuant,
    InformationContentEntity,
    Occurrent,
    Process,
    History,
    TemporalRegion,
}

impl BfoClass {
    pub const fn parent(self) -> Option<Self> {
        match self {
            Self::Entity => None,
            Self::Continuant | Self::Occurrent => Some(Self::Entity),
            Self::IndependentContinuant
            | Self::SpecificallyDependentContinuant
            | Self::GenericallyDependentContinuant => Some(Self::Continuant),
            Self::MaterialEntity | Self::ImmaterialEntity => Some(Self::IndependentContinuant),
            Self::Object => Some(Self::MaterialEntity),
            Self::Site => Some(Self::ImmaterialEntity),
            Self::Role | Self::Disposition | Self::Function | Self::Quality => {
                Some(Self::SpecificallyDependentContinuant)
            }
            Self::InformationContentEntity => Some(Self::GenericallyDependentContinuant),
            Self::Process | Self::History | Self::TemporalRegion => Some(Self::Occurrent),
        }
    }

    pub fn is_a(self, other: Self) -> bool {
        let mut current = Some(self);
        while let Some(class) = current {
            if class == other {
                return true;
            }
            current = class.parent();
        }
        false
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Continuant => "continuant",
            Self::IndependentContinuant => "independent continuant",
            Self::MaterialEntity => "material entity",
            Self::Object => "object",
            Self::ImmaterialEntity => "immaterial entity",
            Self::Site => "site",
            Self::SpecificallyDependentContinuant => "specifically dependent continuant",
            Self::Role => "role",
            Self::Disposition => "disposition",
            Self::Function => "function",
            Self::Quality => "quality",
            Self::GenericallyDependentContinuant => "generically dependent continuant",
            Self::InformationContentEntity => "information content entity",
            Self::Occurrent => "occurrent",
            Self::Process => "process",
            Self::History => "history",
            Self::TemporalRegion => "temporal region",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationKind {
    Contains,
    Occupies,
    ResidentOf,
    ConnectedTo,
    SpecificallyDependsOn,
    InheresIn,
    IsAbout,
    HasParticipant,
    OccursIn,
    HasOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorldNode {
    City(City),
    Place(Place),
    Entity(Entity),
    Npc(Npc),
    Player(Player),
    DependentContinuant(DependentContinuant),
    InformationContent(InformationContent),
    Occurrent(Occurrent),
    TemporalRegion(TemporalRegion),
}

impl WorldNode {
    pub const fn bfo_class(&self) -> BfoClass {
        match self {
            Self::City(_) | Self::Place(_) => BfoClass::Site,
            Self::Entity(_) | Self::Npc(_) | Self::Player(_) => BfoClass::Object,
            Self::DependentContinuant(node) => node.bfo_class(),
            Self::InformationContent(_) => BfoClass::InformationContentEntity,
            Self::Occurrent(_) => BfoClass::Process,
            Self::TemporalRegion(_) => BfoClass::TemporalRegion,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorldEdge {
    TravelRoute(TravelRoute),
    ContainsPlace,
    ContainsEntity,
    ContainsPlayer,
    Resident,
    PresentAt,
    SpecificallyDependsOn,
    InheresIn,
    IsAbout,
    HasParticipant,
    OccursIn,
    HasOutput,
}

impl WorldEdge {
    pub const fn relation_kind(&self) -> RelationKind {
        match self {
            Self::TravelRoute(_) => RelationKind::ConnectedTo,
            Self::ContainsPlace | Self::ContainsEntity | Self::ContainsPlayer => {
                RelationKind::Contains
            }
            Self::Resident => RelationKind::ResidentOf,
            Self::PresentAt => RelationKind::Occupies,
            Self::SpecificallyDependsOn => RelationKind::SpecificallyDependsOn,
            Self::InheresIn => RelationKind::InheresIn,
            Self::IsAbout => RelationKind::IsAbout,
            Self::HasParticipant => RelationKind::HasParticipant,
            Self::OccursIn => RelationKind::OccursIn,
            Self::HasOutput => RelationKind::HasOutput,
        }
    }
}

pub fn add_edge(graph: &mut WorldGraph, a: NodeIndex, b: NodeIndex, relation: WorldEdge) {
    let exists = graph
        .edges_connecting(a, b)
        .any(|edge| edge.weight() == &relation);
    if !exists {
        graph.add_edge(a, b, relation);
    }
}

pub fn edge_snapshot(graph: &WorldGraph) -> Vec<(usize, usize, WorldEdge)> {
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
