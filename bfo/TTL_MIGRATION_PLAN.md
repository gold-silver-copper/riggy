# BFO TTL Parser Migration Plan

This document defines an implementation plan for replacing the current OFN-based parser in [build.rs](/Users/kisaczka/Desktop/code/riggy/bfo/build.rs) with a Turtle-based parser that reads [bfo-core.ttl](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/bfo-core.ttl) and preserves the current generated Rust API.

The current build script reads `BFO-2020-master/21838-2/owl/bfo-core.ofn` and extracts ontology structure using line-oriented pattern matching. The migration target is a TTL pipeline that reconstructs the same semantics from RDF triples, including blank-node class expressions and RDF collections.

## Goals

- switch the BFO build input from `bfo-core.ofn` to `bfo-core.ttl`
- preserve the public generated API and current behavior where possible
- preserve current semantic coverage:
  - class and relation inventories
  - labels, definitions, spec IDs, alt labels, examples, scope notes
  - subclass edges
  - disjointness
  - subproperty edges
  - inverses
  - domain and range expressions
  - property characteristic flags
  - quantified subclass restrictions
- fail loudly on unsupported TTL shapes instead of silently dropping them

## Non-Goals

- full general-purpose OWL reasoning
- immediate CCO support in the same change
- preserving the current OFN parser as a long-term compatibility path
- broad support for every possible RDF serialization

## Current State

The current parser in [build.rs](/Users/kisaczka/Desktop/code/riggy/bfo/build.rs) is organized around OFN surface forms:

- `Declaration(Class(...))`
- `Declaration(ObjectProperty(...))`
- `AnnotationAssertion(...)`
- `SubClassOf(...)`
- `DisjointClasses(...)`
- `SubObjectPropertyOf(...)`
- `InverseObjectProperties(...)`
- `ObjectPropertyDomain(...)`
- `ObjectPropertyRange(...)`
- property-characteristic forms such as `TransitiveObjectProperty(...)`

Those assumptions are embedded directly in helpers such as:

- `parse_declarations`
- `parse_annotations`
- `parse_subclass_axioms`
- `parse_named_binary_axioms`
- `parse_named_group_axioms`
- `parse_inverses`
- `parse_object_property_expr_map`
- `parse_property_flags`

The test suite in [src/lib.rs](/Users/kisaczka/Desktop/code/riggy/bfo/src/lib.rs) is also OFN-coupled and currently validates counts and IDs by scanning `bfo-core.ofn` directly.

## Migration Strategy

Do not rewrite the current OFN string matchers into TTL string matchers. TTL encodes OWL semantics through RDF triples, blank nodes, and RDF lists, so the migration should introduce a graph-based ingestion layer first and only then lift that graph into a small OWL-oriented IR.

Recommended architecture:

1. `parse_ttl_to_graph`
2. `lift_graph_to_ontology_ir`
3. `analyze_ir_into_generated_tables`
4. `render_generated_rust`

This keeps the code generator stable while replacing only the source frontend.

## BFO TTL Shapes That Must Be Supported

The bundled BFO Turtle file uses these patterns and they must be handled explicitly:

- class declarations via `rdf:type owl:Class`
- object property declarations via `rdf:type owl:ObjectProperty`
- annotation properties via `rdf:type owl:AnnotationProperty`
- labels and other text annotations as ordinary RDF triples
- subclass edges via `rdfs:subClassOf`
- disjointness via both:
  - `owl:disjointWith`
  - `owl:AllDisjointClasses` plus `owl:members` RDF lists
- subproperty edges via `rdfs:subPropertyOf`
- inverses via `owl:inverseOf`
- domains via `rdfs:domain`
- ranges via `rdfs:range`
- property flags via `rdf:type` values such as:
  - `owl:TransitiveProperty`
  - `owl:FunctionalProperty`
  - `owl:InverseFunctionalProperty`
- anonymous class expressions represented by blank nodes using:
  - `owl:unionOf`
  - `owl:intersectionOf`
  - `owl:complementOf`
  - `owl:Restriction`
  - `owl:onProperty`
  - `owl:someValuesFrom`
  - `owl:allValuesFrom`

## Phase 1: Freeze Current Semantics

Before changing source format, define exactly what the generated crate must continue to produce.

Tasks:

- enumerate the current semantic outputs produced by [build.rs](/Users/kisaczka/Desktop/code/riggy/bfo/build.rs)
- record a fixture snapshot of the generated class and relation inventories
- record a fixture snapshot of the generated semantic tables that matter:
  - direct parents
  - inverses
  - disjointness
  - domain and range expressions
  - subclass restriction payloads
- document any known OFN-specific output quirks that are acceptable to preserve for now

Exit criteria:

- the migration has a concrete parity target independent of parsing strategy

## Phase 2: Introduce a Syntax-Neutral Ontology IR

Refactor the build script so code generation no longer consumes OFN-specific maps directly.

Tasks:

- add a small ontology IR in [build.rs](/Users/kisaczka/Desktop/code/riggy/bfo/build.rs) or a helper module under `bfo/`
- model:
  - ontology metadata
  - entity declarations
  - annotations
  - class expressions
  - object property expressions
  - axioms
- refactor the code generation pipeline to consume this IR instead of raw OFN helper outputs

Minimum IR capabilities:

- classes
- object properties
- annotation values
- named and anonymous class expressions
- subclass axioms
- disjointness axioms
- inverse axioms
- property domain and range axioms
- property characteristic flags

Exit criteria:

- the existing OFN parser still works, but only as a frontend that populates the new IR

## Phase 3: Add RDF Graph Infrastructure

Add a Turtle parser and a normalized in-memory RDF graph representation.

Preferred approach:

- use a real Turtle parser crate in `build-dependencies`
- store triples in a graph structure with explicit node kinds:
  - IRI node
  - blank node
  - literal node

Tasks:

- choose and add a Turtle parser crate
- parse `bfo-core.ttl` into a graph
- normalize prefixed names to full IRIs
- preserve literal language tags and datatypes
- add helpers to query outgoing and incoming triples by subject and predicate

Important constraint:

- the graph layer must not interpret OWL yet; it should only represent RDF structure faithfully

Exit criteria:

- a parsed BFO graph can be traversed deterministically in tests

## Phase 4: Implement RDF Collection Decoding

TTL uses RDF lists for `owl:unionOf`, `owl:intersectionOf`, and `owl:members`.

Tasks:

- implement `read_rdf_list(node) -> Vec<Node>`
- validate proper `rdf:first` / `rdf:rest` structure
- detect cycles, malformed lists, duplicate tail branches, or missing `rdf:nil`
- surface build errors with enough source context to diagnose malformed data

This phase is required before decoding:

- unions
- intersections
- `owl:AllDisjointClasses`

Exit criteria:

- RDF lists are decoded once and reused by all higher-level lifting code

## Phase 5: Lift RDF Graph To OWL-Oriented IR

Translate BFO TTL graph patterns into the syntax-neutral IR.

Tasks:

- lift named class declarations from `rdf:type owl:Class`
- lift named object property declarations from `rdf:type owl:ObjectProperty`
- collect annotations from RDF predicates used in the current generator
- lift `rdfs:subClassOf` triples into subclass axioms
- lift `rdfs:subPropertyOf` triples into subproperty axioms
- lift `owl:inverseOf` into inverse axioms
- lift `rdfs:domain` and `rdfs:range` into property expression axioms
- lift property characteristic flags from `rdf:type`
- lift `owl:disjointWith`
- lift `owl:AllDisjointClasses` via `owl:members`

For blank-node class expressions, implement recursive decoding:

- named class
- union
- intersection
- complement
- restriction with `owl:onProperty`
- `owl:someValuesFrom`
- `owl:allValuesFrom`

Validation rules:

- reject unsupported blank-node shapes
- reject restriction nodes with missing or multiple fillers
- reject unsupported property expressions if encountered

Exit criteria:

- `lift_graph_to_ontology_ir` produces enough data to drive the existing renderer without OFN

## Phase 6: Add Dual-Frontend Parity Tests

Before cutting over the build script, prove that TTL lifting matches the current OFN-derived behavior.

Tasks:

- keep the OFN frontend temporarily
- add tests that parse both:
  - `bfo-core.ofn`
  - `bfo-core.ttl`
- compare normalized IR outputs for:
  - declared classes
  - declared relations
  - annotations
  - class parent edges
  - subproperty edges
  - inverse links
  - disjoint sets
  - domain and range expressions
  - subclass restrictions
  - property characteristic flags

Important note:

- some OFN and TTL forms may differ structurally while remaining semantically equivalent
- comparisons should happen on normalized IR, not on raw syntax fragments

Exit criteria:

- TTL and OFN frontends produce equivalent normalized data for the currently supported BFO source set

## Phase 7: Cut Over The Build Script To TTL

Once parity is proven, switch the build script to use Turtle as the canonical input.

Tasks:

- replace the source path in [build.rs](/Users/kisaczka/Desktop/code/riggy/bfo/build.rs) from `bfo-core.ofn` to `bfo-core.ttl`
- emit `cargo:rerun-if-changed` for the TTL file
- route generation through the TTL frontend
- keep the OFN frontend only if needed for parity tests during the transition

Exit criteria:

- normal `cargo build` and `cargo test` for the crate no longer depend on parsing OFN

## Phase 8: Rewrite Test Fixtures Around TTL Or IR

The runtime tests in [src/lib.rs](/Users/kisaczka/Desktop/code/riggy/bfo/src/lib.rs) currently scan OFN directly. Those tests should be rewritten so they validate generated output against TTL-derived facts or against normalized IR fixtures.

Tasks:

- replace OFN line-count tests with TTL/IR-based inventory checks
- verify all current API-level tests still pass
- add tests for TTL-only structures that were not explicit in the OFN-oriented suite:
  - `owl:AllDisjointClasses`
  - RDF-list decoding
  - blank-node restriction decoding

Exit criteria:

- the test suite no longer depends on scanning `bfo-core.ofn`

## Phase 9: Remove OFN-Specific Parser Code

After TTL cutover and parity validation, delete the OFN-specific machinery rather than carrying both indefinitely.

Tasks:

- remove OFN parsing helpers
- remove OFN-specific fixtures and test utilities
- simplify generator internals around the syntax-neutral IR plus TTL frontend
- update [IMPLEMENTATION_PLAN.md](/Users/kisaczka/Desktop/code/riggy/bfo/IMPLEMENTATION_PLAN.md) to describe TTL as the active source format

Exit criteria:

- the build pipeline has one canonical source frontend for BFO

## Suggested Execution Order

1. Freeze current outputs.
2. Introduce the IR.
3. Refactor code generation to consume the IR.
4. Add the RDF graph layer and TTL parser.
5. Implement RDF-list decoding.
6. Lift TTL graph shapes into the IR.
7. Add OFN-vs-TTL parity tests.
8. Switch the build to TTL.
9. Rewrite tests to stop scanning OFN.
10. Delete OFN parsing code.

## Risks

### Risk: Silent semantic loss during blank-node decoding

Mitigation:

- require explicit pattern matches for every supported OWL shape
- treat unknown shapes as hard build errors

### Risk: RDF-list bugs cause incomplete unions or disjoint sets

Mitigation:

- centralize RDF-list decoding
- test malformed and valid list cases directly

### Risk: TTL and OFN encode semantically equivalent data with different local structure

Mitigation:

- compare normalized IR outputs instead of syntax fragments

### Risk: The migration becomes entangled with CCO support

Mitigation:

- keep the first migration strictly scoped to BFO core TTL parity
- only add CCO after the TTL frontend is stable

## Acceptance Criteria

This migration is successful when:

- [build.rs](/Users/kisaczka/Desktop/code/riggy/bfo/build.rs) reads `bfo-core.ttl` instead of `bfo-core.ofn`
- the generated public API remains intact unless an intentional cleanup is made
- generated inventories and semantic tables match the current BFO core outputs
- blank-node class expressions are decoded into typed IR expressions
- `owl:AllDisjointClasses` is preserved as generated disjointness data
- tests no longer depend on scanning OFN text
- unsupported TTL shapes fail the build explicitly

## Recommended First Commit Slice

The first implementation slice should not attempt the whole migration at once.

Recommended first slice:

- introduce the syntax-neutral IR
- route the current OFN parser through that IR
- add parity-focused snapshot tests around the IR

That reduces risk before any Turtle parsing work begins.
