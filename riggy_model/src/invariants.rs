use bfo::RelationKind;
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use riggy_ontology::relation::RiggyRelation;

use crate::graph_ecs::{CityId, EntityId, NpcId, PlaceId, PlayerId, ProcessId};
use crate::graph_ecs::{WorldEdge, WorldNode, WorldRelation};
use crate::world::OccurrentKind;
use crate::world::World;

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
    PlayerMultiplePresentAtPlaces {
        player_id: PlayerId,
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
    PlayerMultipleContainers {
        player_id: PlayerId,
        count: usize,
    },
    CurrentTimeMissing,
    CurrentTimeMultiple {
        count: usize,
    },
    PlayerMultipleActiveDialogues {
        player_id: PlayerId,
        count: usize,
    },
    DialogueMissingNpcParticipant {
        process_id: ProcessId,
    },
    DialogueMultipleNpcParticipants {
        process_id: ProcessId,
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
    InvalidOntologyRelation {
        relation: WorldRelation,
        from: usize,
        to: usize,
    },
    MissingSymmetricRelation {
        relation: WorldRelation,
        from: usize,
        to: usize,
    },
}

pub fn validate_world(world: &World) -> Vec<InvariantViolation> {
    let mut violations = Vec::new();
    let current_time_count = world
        .graph
        .node_indices()
        .filter(|index| {
            matches!(
                world.graph.node_weight(*index),
                Some(WorldNode::TemporalRegion(_))
            )
        })
        .count();
    match current_time_count {
        1 => {}
        0 => violations.push(InvariantViolation::CurrentTimeMissing),
        count => violations.push(InvariantViolation::CurrentTimeMultiple { count }),
    }

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
                    _ => {}
                }
            }
            Some(WorldNode::Npc(_)) => {
                let resident_cities = world.npc_resident_city_ids(NpcId(index));
                match resident_cities.len() {
                    1 => {}
                    0 => violations.push(InvariantViolation::NpcMissingResidentCity {
                        npc_id: NpcId(index),
                    }),
                    _ => {}
                }

                let present_at_places = world.npc_present_place_ids(NpcId(index));
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
            Some(WorldNode::Entity(_))
            | Some(WorldNode::DependentContinuant(_))
            | Some(WorldNode::InformationContent(_))
            | Some(WorldNode::TemporalRegion(_)) => {}
            Some(WorldNode::Player(_)) => {
                let active_dialogues = world.active_dialogue_process_ids(PlayerId(index));
                if active_dialogues.len() > 1 {
                    violations.push(InvariantViolation::PlayerMultipleActiveDialogues {
                        player_id: PlayerId(index),
                        count: active_dialogues.len(),
                    });
                }
            }
            Some(WorldNode::Occurrent(process)) => {
                if matches!(process.kind, OccurrentKind::Dialogue) {
                    let npc_count = world
                        .graph
                        .edges_directed(index, petgraph::Direction::Outgoing)
                        .filter(|edge| {
                            edge.weight().relation()
                                == WorldRelation::Bfo(RelationKind::HasParticipant)
                        })
                        .filter(|edge| {
                            matches!(
                                world.graph.node_weight(edge.target()),
                                Some(WorldNode::Npc(_))
                            )
                        })
                        .count();
                    match npc_count {
                        1 => {}
                        0 => violations.push(InvariantViolation::DialogueMissingNpcParticipant {
                            process_id: ProcessId(index),
                        }),
                        count => {
                            violations.push(InvariantViolation::DialogueMultipleNpcParticipants {
                                process_id: ProcessId(index),
                                count,
                            })
                        }
                    }
                }
            }
            None => {}
        }
    }

    for edge in world.graph.edge_references() {
        if !relation_endpoints_are_valid(world, edge.weight(), edge.source(), edge.target()) {
            violations.push(invalid_edge_violation(
                edge.weight(),
                edge.source(),
                edge.target(),
            ));
        }
        if let Some(kind) = edge.weight().bfo_relation_kind() {
            if kind.is_symmetric()
                && !has_symmetric_peer(world, edge.weight(), edge.source(), edge.target())
            {
                violations.push(InvariantViolation::MissingSymmetricRelation {
                    relation: WorldRelation::Bfo(kind),
                    from: edge.source().index(),
                    to: edge.target().index(),
                });
            }
        } else if edge.weight().relation() == WorldRelation::Riggy(RiggyRelation::TravelRoute)
            && !has_symmetric_peer(world, edge.weight(), edge.source(), edge.target())
        {
            violations.push(InvariantViolation::MissingSymmetricRelation {
                relation: WorldRelation::Riggy(RiggyRelation::TravelRoute),
                from: edge.source().index(),
                to: edge.target().index(),
            });
        }
    }

    violations.extend(validate_target_cardinality(
        world,
        WorldRelation::Riggy(RiggyRelation::Contains),
        1,
    ));
    violations.extend(validate_target_cardinality(
        world,
        WorldRelation::Riggy(RiggyRelation::PresentAt),
        1,
    ));
    violations.extend(validate_target_cardinality(
        world,
        WorldRelation::Riggy(RiggyRelation::ResidentOf),
        1,
    ));

    violations
}

fn relation_endpoints_are_valid(
    world: &World,
    edge: &WorldEdge,
    source: NodeIndex,
    target: NodeIndex,
) -> bool {
    match edge.bfo_relation_kind() {
        Some(kind) => {
            let Some(source_class) = world.bfo_class(source) else {
                return false;
            };
            let Some(target_class) = world.bfo_class(target) else {
                return false;
            };
            kind.domain_allows(source_class) && kind.range_allows(target_class)
        }
        None => match edge.relation() {
            WorldRelation::Riggy(RiggyRelation::TravelRoute) => {
                matches!(
                    world.graph.node_weight(source),
                    Some(WorldNode::City(_) | WorldNode::Place(_))
                ) && matches!(
                    world.graph.node_weight(target),
                    Some(WorldNode::City(_) | WorldNode::Place(_))
                )
            }
            WorldRelation::Riggy(RiggyRelation::Contains) => {
                matches!(
                    world.graph.node_weight(source),
                    Some(WorldNode::City(_) | WorldNode::Place(_) | WorldNode::Entity(_))
                ) && matches!(
                    world.graph.node_weight(target),
                    Some(WorldNode::Place(_) | WorldNode::Entity(_) | WorldNode::Player(_))
                )
            }
            WorldRelation::Riggy(RiggyRelation::ResidentOf) => {
                matches!(world.graph.node_weight(source), Some(WorldNode::City(_)))
                    && matches!(world.graph.node_weight(target), Some(WorldNode::Npc(_)))
            }
            WorldRelation::Riggy(RiggyRelation::PresentAt) => {
                matches!(world.graph.node_weight(source), Some(WorldNode::Place(_)))
                    && matches!(
                        world.graph.node_weight(target),
                        Some(WorldNode::Npc(_) | WorldNode::Player(_))
                    )
            }
            WorldRelation::Riggy(RiggyRelation::IsAbout) => {
                matches!(
                    world.graph.node_weight(source),
                    Some(WorldNode::InformationContent(_))
                ) && matches!(
                    world.graph.node_weight(target),
                    Some(
                        WorldNode::City(_)
                            | WorldNode::Npc(_)
                            | WorldNode::Occurrent(_)
                            | WorldNode::Player(_)
                    )
                )
            }
            WorldRelation::Riggy(RiggyRelation::HasOutput) => {
                matches!(
                    world.graph.node_weight(source),
                    Some(WorldNode::Occurrent(_))
                ) && matches!(
                    world.graph.node_weight(target),
                    Some(WorldNode::InformationContent(_))
                )
            }
            WorldRelation::Bfo(_) => unreachable!("handled above"),
        },
    }
}

fn has_symmetric_peer(
    world: &World,
    edge: &WorldEdge,
    source: NodeIndex,
    target: NodeIndex,
) -> bool {
    world
        .graph
        .edges_connecting(target, source)
        .any(|candidate| candidate.weight() == edge)
}

fn invalid_edge_violation(
    edge: &WorldEdge,
    source: NodeIndex,
    target: NodeIndex,
) -> InvariantViolation {
    match edge {
        WorldEdge::TravelRoute(_) => InvariantViolation::InvalidTravelRouteEndpoints {
            from: source.index(),
            to: target.index(),
        },
        WorldEdge::ContainsPlace => InvariantViolation::InvalidContainsPlaceEdge {
            city_id: CityId(source),
            target: target.index(),
        },
        WorldEdge::ContainsEntity => InvariantViolation::InvalidContainsEntityEdge {
            place_id: PlaceId(source),
            target: target.index(),
        },
        WorldEdge::Resident => InvariantViolation::InvalidResidentEdge {
            city_id: CityId(source),
            target: target.index(),
        },
        WorldEdge::PresentAt => InvariantViolation::InvalidPresentAtEdge {
            place_id: PlaceId(source),
            target: target.index(),
        },
        WorldEdge::SpecificallyDependsOn
        | WorldEdge::InheresIn
        | WorldEdge::IsAbout
        | WorldEdge::HasParticipant
        | WorldEdge::OccursIn
        | WorldEdge::HasOutput => InvariantViolation::InvalidOntologyRelation {
            relation: edge.relation(),
            from: source.index(),
            to: target.index(),
        },
    }
}

fn validate_target_cardinality(
    world: &World,
    relation: WorldRelation,
    max_incoming: usize,
) -> Vec<InvariantViolation> {
    let mut violations = Vec::new();

    for index in world.graph.node_indices() {
        let count = world
            .graph
            .edges_directed(index, petgraph::Direction::Incoming)
            .filter(|edge| edge.weight().relation() == relation)
            .count();
        if count <= max_incoming {
            continue;
        }
        match relation {
            WorldRelation::Riggy(RiggyRelation::Contains) => match world.graph.node_weight(index) {
                Some(WorldNode::Place(_)) => {
                    violations.push(InvariantViolation::PlaceMultipleCities {
                        place_id: PlaceId(index),
                        count,
                    });
                }
                Some(WorldNode::Entity(_)) => {
                    violations.push(InvariantViolation::EntityMultipleContainers {
                        entity_id: EntityId(index),
                        count,
                    });
                }
                Some(WorldNode::Player(_)) => {
                    violations.push(InvariantViolation::PlayerMultipleContainers {
                        player_id: PlayerId(index),
                        count,
                    });
                }
                _ => {}
            },
            WorldRelation::Riggy(RiggyRelation::PresentAt) => {
                match world.graph.node_weight(index) {
                    Some(WorldNode::Npc(_)) => {
                        violations.push(InvariantViolation::NpcMultiplePresentAtPlaces {
                            npc_id: NpcId(index),
                            count,
                        });
                    }
                    Some(WorldNode::Player(_)) => {
                        violations.push(InvariantViolation::PlayerMultiplePresentAtPlaces {
                            player_id: PlayerId(index),
                            count,
                        });
                    }
                    _ => {}
                }
            }
            WorldRelation::Riggy(RiggyRelation::ResidentOf) => {
                if matches!(world.graph.node_weight(index), Some(WorldNode::Npc(_))) {
                    violations.push(InvariantViolation::NpcMultipleResidentCities {
                        npc_id: NpcId(index),
                        count,
                    });
                }
            }
            WorldRelation::Riggy(RiggyRelation::TravelRoute)
            | WorldRelation::Riggy(RiggyRelation::IsAbout)
            | WorldRelation::Riggy(RiggyRelation::HasOutput)
            | WorldRelation::Bfo(_) => {}
        }
    }

    violations
}
