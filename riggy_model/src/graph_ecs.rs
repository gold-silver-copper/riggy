use bfo::{BfoClass, RelationKind};
use petgraph::Directed;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use riggy_ontology::relation::RiggyRelation;
use serde::{Deserialize, Serialize};

use crate::world::{
    City, DependentContinuant, Entity, InformationContent, Npc, Occurrent, Place, Player,
    TemporalRegion, TravelRoute,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CityId(pub NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NpcId(pub NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaceId(pub NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(pub NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub NodeIndex);

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
pub enum WorldRelation {
    Bfo(RelationKind),
    Riggy(RiggyRelation),
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
    pub const fn relation(&self) -> WorldRelation {
        match self {
            Self::TravelRoute(_) => WorldRelation::Riggy(RiggyRelation::TravelRoute),
            Self::ContainsPlace | Self::ContainsEntity | Self::ContainsPlayer => {
                WorldRelation::Riggy(RiggyRelation::Contains)
            }
            Self::Resident => WorldRelation::Riggy(RiggyRelation::ResidentOf),
            Self::PresentAt => WorldRelation::Riggy(RiggyRelation::PresentAt),
            Self::SpecificallyDependsOn => WorldRelation::Bfo(RelationKind::SpecificallyDependsOn),
            Self::InheresIn => WorldRelation::Bfo(RelationKind::InheresIn),
            Self::IsAbout => WorldRelation::Riggy(RiggyRelation::IsAbout),
            Self::HasParticipant => WorldRelation::Bfo(RelationKind::HasParticipant),
            Self::OccursIn => WorldRelation::Bfo(RelationKind::OccursIn),
            Self::HasOutput => WorldRelation::Riggy(RiggyRelation::HasOutput),
        }
    }

    pub const fn bfo_relation_kind(&self) -> Option<RelationKind> {
        match self.relation() {
            WorldRelation::Bfo(kind) => Some(kind),
            WorldRelation::Riggy(_) => None,
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
