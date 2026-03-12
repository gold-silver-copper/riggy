use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::graph_ecs::{CityId, EntityId, NpcId, PlaceId};
use crate::graph_ecs::{WorldEdge, WorldNode};
use crate::world::{NodeKind, World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantViolation {
    PlaceMissingCity {
        place_id: PlaceId,
    },
    PlaceMultipleCities {
        place_id: PlaceId,
        count: usize,
    },
    NpcMissingResidentCity {
        npc_id: NpcId,
    },
    NpcMultipleResidentCities {
        npc_id: NpcId,
        count: usize,
    },
    NpcMultiplePresentAtPlaces {
        npc_id: NpcId,
        count: usize,
    },
    NpcPresentOutsideResidentCity {
        npc_id: NpcId,
        resident_city_id: CityId,
        present_city_id: CityId,
    },
    EntityMultipleContainers {
        entity_id: EntityId,
        count: usize,
    },
    InvalidTravelRouteEndpoints {
        from: usize,
        to: usize,
    },
    InvalidContainsPlaceEdge {
        city_id: CityId,
        target: usize,
    },
    InvalidContainsEntityEdge {
        place_id: PlaceId,
        target: usize,
    },
    InvalidResidentEdge {
        city_id: CityId,
        target: usize,
    },
    InvalidPresentAtEdge {
        place_id: PlaceId,
        target: usize,
    },
}

pub fn validate_world(world: &World) -> Vec<InvariantViolation> {
    let mut violations = Vec::new();

    for index in world.graph.node_indices() {
        match world.graph.node_weight(index) {
            Some(WorldNode::City(_)) => {}
            Some(WorldNode::Place(_)) => {
                let city_count = world.place_city_ids(PlaceId(index)).len();
                match city_count {
                    1 => {}
                    0 => violations.push(InvariantViolation::PlaceMissingCity {
                        place_id: PlaceId(index),
                    }),
                    count => violations.push(InvariantViolation::PlaceMultipleCities {
                        place_id: PlaceId(index),
                        count,
                    }),
                }
            }
            Some(WorldNode::Npc(_)) => {
                let resident_cities = world.npc_resident_city_ids(NpcId(index));
                match resident_cities.len() {
                    1 => {}
                    0 => violations.push(InvariantViolation::NpcMissingResidentCity {
                        npc_id: NpcId(index),
                    }),
                    count => violations.push(InvariantViolation::NpcMultipleResidentCities {
                        npc_id: NpcId(index),
                        count,
                    }),
                }

                let present_at_places = world.npc_present_place_ids(NpcId(index));
                if present_at_places.len() > 1 {
                    violations.push(InvariantViolation::NpcMultiplePresentAtPlaces {
                        npc_id: NpcId(index),
                        count: present_at_places.len(),
                    });
                }
                if let (Some(resident_city_id), Some(present_place_id)) = (
                    resident_cities.first().copied(),
                    present_at_places.first().copied(),
                ) {
                    if let Some(present_city_id) = world.place_city_id(present_place_id) {
                        if resident_city_id != present_city_id {
                            violations.push(InvariantViolation::NpcPresentOutsideResidentCity {
                                npc_id: NpcId(index),
                                resident_city_id,
                                present_city_id,
                            });
                        }
                    }
                }
            }
            Some(WorldNode::Entity(_)) => {
                let container_count = world.entity_container_place_ids(EntityId(index)).len();
                if container_count > 1 {
                    violations.push(InvariantViolation::EntityMultipleContainers {
                        entity_id: EntityId(index),
                        count: container_count,
                    });
                }
            }
            None => {}
        }
    }

    for edge in world.graph.edge_references() {
        match edge.weight() {
            WorldEdge::TravelRoute(_) => {
                let valid = edge_matches_either(
                    world,
                    edge.source(),
                    edge.target(),
                    (NodeKind::City, NodeKind::City),
                    (NodeKind::Place, NodeKind::Place),
                );
                if !valid {
                    violations.push(InvariantViolation::InvalidTravelRouteEndpoints {
                        from: edge.source().index(),
                        to: edge.target().index(),
                    });
                }
            }
            WorldEdge::ContainsPlace => {
                if !edge_matches(world, edge.source(), edge.target(), NodeKind::City, NodeKind::Place)
                {
                    violations.push(InvariantViolation::InvalidContainsPlaceEdge {
                        city_id: CityId(edge.source()),
                        target: edge.target().index(),
                    });
                }
            }
            WorldEdge::ContainsEntity => {
                if !edge_matches(
                    world,
                    edge.source(),
                    edge.target(),
                    NodeKind::Place,
                    NodeKind::Entity,
                ) {
                    violations.push(InvariantViolation::InvalidContainsEntityEdge {
                        place_id: PlaceId(edge.source()),
                        target: edge.target().index(),
                    });
                }
            }
            WorldEdge::Resident => {
                if !edge_matches(world, edge.source(), edge.target(), NodeKind::City, NodeKind::Npc)
                {
                    violations.push(InvariantViolation::InvalidResidentEdge {
                        city_id: CityId(edge.source()),
                        target: edge.target().index(),
                    });
                }
            }
            WorldEdge::PresentAt => {
                if !edge_matches(world, edge.source(), edge.target(), NodeKind::Place, NodeKind::Npc)
                {
                    violations.push(InvariantViolation::InvalidPresentAtEdge {
                        place_id: PlaceId(edge.source()),
                        target: edge.target().index(),
                    });
                }
            }
        }
    }

    violations
}

fn edge_matches(
    world: &World,
    source: petgraph::stable_graph::NodeIndex,
    target: petgraph::stable_graph::NodeIndex,
    expected_source: NodeKind,
    expected_target: NodeKind,
) -> bool {
    world.node_kind(source) == Some(expected_source) && world.node_kind(target) == Some(expected_target)
}

fn edge_matches_either(
    world: &World,
    source: petgraph::stable_graph::NodeIndex,
    target: petgraph::stable_graph::NodeIndex,
    a: (NodeKind, NodeKind),
    b: (NodeKind, NodeKind),
) -> bool {
    edge_matches(world, source, target, a.0, a.1)
        || edge_matches(world, source, target, b.0, b.1)
}
