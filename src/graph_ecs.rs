use petgraph::Directed;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use serde::{Deserialize, Serialize};

use crate::world::{City, Entity, Npc, Place, TravelRoute};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CityId(pub(crate) NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NpcId(pub(crate) NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaceId(pub(crate) NodeIndex);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(pub(crate) NodeIndex);

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

pub type WorldGraph = StableGraph<WorldNode, WorldEdge, Directed>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorldNode {
    City(City),
    Place(Place),
    Entity(Entity),
    Npc(Npc),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorldEdge {
    TravelRoute(TravelRoute),
    ContainsPlace,
    ContainsEntity,
    Resident,
    PresentAt,
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
