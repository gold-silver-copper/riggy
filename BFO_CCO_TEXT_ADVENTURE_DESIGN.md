# BFO 2020 + CCO Traversal Architecture

## Purpose

This document describes a minimal architecture for a text adventure world model that uses only BFO 2020 and CCO terms, with `petgraph` as the authoritative data store.

The scope is intentionally narrow:

- one player
- a set of places
- movement between places
- a keybinding-driven travel menu

This is an architectural document, not an implementation plan. It does not cover combat, inventory, quests, procedural text generation, or any custom game ontology.

## Architectural Position

The world is a graph of uniform nodes and uniform edges.

Nodes are not typed by Rust enums or by distinct node variants such as `PlaceNode` or `PersonNode`. Instead, a node carries one or more ontology term identifiers, and the system decides what the node is by comparing those identifiers against BFO or CCO identifiers.

The same rule applies to relations. An edge is not a bespoke `TravelEdge` or `LocationEdge`. It carries the identifier of a BFO or CCO relation, and graph operations interpret the edge by comparing that relation identifier.

The engine therefore reasons over ontology identifiers rather than over application-defined structural types.

## Ontology Terms in Scope

For the current scope, only a very small subset of the vendored ontologies is needed.

Primary class terms:

- `BFO_0000029` `site`
- `cco:ont00001262` `Person`

Primary relation terms:

- `BFO_0000171` `located in`
- `cco:ont00001810` `connected with`

These are present in the local sources:

- [bfo-core.ttl](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/bfo-core.ttl)
- [AgentOntology.ttl](/Users/kisaczka/Desktop/code/riggy/bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl)
- [GeospatialOntology.ttl](/Users/kisaczka/Desktop/code/riggy/bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl)

## Minimal Semantic Model

### Player

The player is represented by a single node that carries the CCO identifier for `Person`.

At this stage, no further modeling is required. The player is simply the controlled entity whose current place is tracked through graph relations.

### Places

Every traversable location is represented as a node carrying the BFO identifier for `site`.

To keep the architecture simple, all current places are modeled as sites. This avoids introducing additional place categories such as facility, geospatial region, building, room, or settlement before they are actually needed.

### Current Position

The player's current position is represented by a `located in` edge from the player node to exactly one site node.

This gives the architecture a single authoritative answer to the question, "Where is the player now?"

### Place Connectivity

Connectivity between places is represented by `connected with` edges between site nodes.

For the current document, that relation is used as the source of travel options. If two site nodes are connected by `connected with`, then the travel menu may offer one as a destination from the other.

This is intentionally conservative. The document does not add any custom relation such as `adjacent to`, `reachable from`, or `traversable to`.

## Graph Shape

Conceptually, each graph node should contain:

- a stable runtime node identity
- one or more asserted ontology class identifiers
- minimal descriptive metadata needed for UI presentation

Conceptually, each graph edge should contain:

- a stable runtime edge identity
- one asserted ontology relation identifier
- source node identity
- target node identity

The important architectural point is that class and relation meaning come from ontology identifiers stored in data, not from hard-coded node kinds.

## Classification Rules

The world model should answer questions through ontology ID comparison.

Examples:

- a node is treated as a place if it carries `BFO_0000029`
- a node is treated as the player character if it is the controlled node and carries `cco:ont00001262`
- an edge is treated as a location relation if it carries `BFO_0000171`
- an edge is treated as a place-connection relation if it carries `cco:ont00001810`

This keeps the architecture aligned with your constraint that typing should be done through BFO or CCO identifiers rather than through separate application-specific node types.

## Travel Menu Architecture

The control scheme is keybinding-first.

For the current scope, the main action is:

- `T` opens the travel menu

The travel menu is derived entirely from the graph:

1. Start from the controlled player node.
2. Follow its `located in` edge to the current site.
3. Query all `connected with` edges attached to that current site.
4. Resolve the opposite endpoint of each such edge.
5. Keep only endpoints that carry the `site` identifier.
6. Present those site nodes as travel destinations.

No separate travel table, room adjacency matrix, or menu-specific state store is needed. The menu is a view over the current graph.

## Travel State Change

When the user selects a destination from the travel menu, the world state change is minimal:

1. Remove the player's current `located in` edge.
2. Create a new `located in` edge from the player node to the selected destination site.

That edge update is the authoritative movement operation.

Everything else in the interface should derive from that new fact.

## Required Invariants

For this minimal architecture to remain coherent, a small set of invariants should hold at all times:

- the controlled player node carries the `Person` identifier
- every traversable place node carries the `site` identifier
- the player has exactly one outgoing `located in` edge
- the target of the player's `located in` edge is a site
- every place shown in the travel menu is connected to the current site through `connected with`

These invariants are enough to keep the travel loop well-defined without introducing broader game systems.

## Intentional Limits

This design deliberately does not address:

- whether `connected with` should later be refined into a stricter travel relation
- blocked travel, locked passages, hazards, or directional movement
- process modeling for movement acts
- pathfinding beyond immediate neighboring places
- narrative text generation beyond displaying place labels
- any ontology terms outside the currently necessary BFO and CCO subset

Those concerns should stay out of the architecture until basic place traversal is stable.

## Semantic Tradeoff

Using only BFO and CCO keeps the model disciplined, but it also means the current travel architecture is intentionally simple.

`connected with` is the closest available relation in the currently chosen scope for connecting sites without inventing a game-specific ontology. In a richer system, you might want to distinguish mere topological connection from allowed player movement. This document does not do that. For now, if two sites are connected in the graph by the CCO relation, that connection is treated as a valid travel option.

## Architectural Summary

The minimal architecture is:

- one `Person` node for the player
- many `site` nodes for places
- one `located in` edge from player to current site
- `connected with` edges between sites
- a `T` travel menu generated by traversing those ontology-labeled edges

The key principle is that neither nodes nor edges are given meaning by application-specific types. Meaning comes from stored BFO and CCO identifiers, and game logic operates by comparing those identifiers.

## Local Source References

- BFO `site`: [bfo-core.ttl](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/bfo-core.ttl)
- BFO `located in`: [bfo-core.ttl](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/bfo-core.ttl)
- CCO `Person`: [AgentOntology.ttl](/Users/kisaczka/Desktop/code/riggy/bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl)
- CCO `connected with`: [GeospatialOntology.ttl](/Users/kisaczka/Desktop/code/riggy/bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl)
