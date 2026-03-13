# BFO TTL-Only Destructive Rewrite Plan

This plan assumes a deliberate hard cut:

- delete the OFN parser
- stop reading `bfo-core.ofn`
- stop validating against OFN text
- rebuild the generator around `bfo-core.ttl` as the only source of truth

This is not a migration plan in the compatibility sense. It is a rewrite plan that treats the current OFN-based build pipeline as disposable.

## Rewrite Principles

- TTL is the only ontology input format for `bfo`
- `oxrdfio` and `oxrdf` are the standard ingestion stack
- existing OFN parsing helpers are not preserved behind flags or fallback paths
- current parser-specific tests are replaced, not adapted
- semantic parity matters more than implementation continuity
- temporary breakage during the rewrite is acceptable
- RDF is a build-time intermediate, not the final product
- the real output is typed Rust code generated from ontology semantics

## Hard Cut Scope

Delete or replace all OFN-coupled assumptions in:

- [build.rs](/Users/kisaczka/Desktop/code/riggy/bfo/build.rs)
- [src/lib.rs](/Users/kisaczka/Desktop/code/riggy/bfo/src/lib.rs)
- [IMPLEMENTATION_PLAN.md](/Users/kisaczka/Desktop/code/riggy/bfo/IMPLEMENTATION_PLAN.md)

Retain only:

- the generated public Rust API, if it remains a useful surface
- the bundled BFO Turtle artifact at [bfo-core.ttl](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/bfo-core.ttl)

Everything else is negotiable.

## Target End State

The crate should have this pipeline:

1. parse Turtle with `oxrdfio`
2. store and index RDF triples with `oxrdf`
3. decode RDF collections and blank-node OWL structures
4. lift RDF into a typed ontology IR
5. derive generated Rust tables and enums from the IR
6. render `generated.rs`

There should be no OFN code, no OFN fixtures, and no runtime tests that read OFN files.

## Product Focus: Typed Rust, Not RDF

The success condition is not "we can parse Turtle".

The success condition is that the build script emits strongly typed Rust code such as:

- `enum BfoClass`
- `enum RelationKind`
- typed semantic helper enums and structs
- generated lookup functions by stable identifiers
- generated semantic tables for hierarchy, inverses, domains, ranges, disjointness, and restrictions

The RDF graph and OWL lifting layers are internal machinery only. They must not leak into the public crate API.

## Phase 0: Branch The Rewrite

Do not attempt this incrementally inside the current parser structure.

Tasks:

- create a dedicated rewrite branch
- treat `build.rs` as a replacement target, not a refactor target
- freeze the current generated output only long enough to compare semantics during the rewrite

Deliverable:

- a branch where large-scale file replacement is expected

## Phase 1: Delete OFN As An Input Concept

Start by removing the assumption that OFN exists at all.

Tasks:

- replace every `bfo-core.ofn` path reference with `bfo-core.ttl`
- delete OFN-specific helper names and terminology from planning docs
- mark any OFN-driven tests as obsolete
- remove comments and docs that frame OFN as canonical

Files likely affected:

- [build.rs](/Users/kisaczka/Desktop/code/riggy/bfo/build.rs)
- [src/lib.rs](/Users/kisaczka/Desktop/code/riggy/bfo/src/lib.rs)
- [IMPLEMENTATION_PLAN.md](/Users/kisaczka/Desktop/code/riggy/bfo/IMPLEMENTATION_PLAN.md)

Acceptance criteria:

- no production code references `bfo-core.ofn`

## Phase 2: Replace `build.rs` With A New Architecture

Do not keep the current map-of-special-cases design.

Replace it with explicit layers:

```rust
fn main() {
    let graph = parse_ttl_to_graph(ttl_path());
    let ontology = lift_graph_to_ontology(graph);
    let typed_model = analyze_ontology(&ontology);
    let generated = render_generated_rust(&typed_model);
    write_output(generated);
}
```

Required internal modules or sections:

- source loading
- RDF graph model
- RDF list decoder
- OWL expression decoder
- ontology IR
- typed code analysis layer
- Rust renderer

Acceptance criteria:

- no function in the new build script parses syntax by looking for OFN prefixes

## Phase 3: Introduce A Real RDF Graph Layer

The rewrite must stop pretending TTL is line-oriented text.

Tasks:

- add `oxrdfio` under `build-dependencies`
- add `oxrdf` under `build-dependencies`
- parse into explicit node types:
  - IRI
  - blank node
  - literal
- store triples in indexed collections for deterministic traversal
- normalize predicates and object IRIs to full strings

Recommended stack:

- `oxrdfio` for Turtle parsing
- `oxrdf` for RDF terms and triples
- custom indexing structures on top of `oxrdf` types for efficient OWL lifting

Graph requirements:

- deterministic iteration order
- exact preservation of blank-node identity within one parse
- access helpers for:
  - all triples by subject
  - all objects by `(subject, predicate)`
  - all subjects by `(predicate, object)`

Acceptance criteria:

- the build script can query BFO TTL as RDF rather than text
- no custom Turtle tokenizer or ad hoc TTL parser exists in the codebase

## Phase 4: Implement RDF Collection Decoding First

This is a core prerequisite, not an optional utility.

TTL BFO uses RDF lists for:

- `owl:unionOf`
- `owl:intersectionOf`
- `owl:members`

Tasks:

- implement a strict RDF-list decoder
- reject malformed collection structures
- produce stable ordered vectors from RDF lists

Failure policy:

- any malformed RDF list aborts the build

Acceptance criteria:

- lists are decoded through one canonical function used everywhere

## Phase 5: Build A Typed OWL IR

The new IR should be syntax-neutral and RDF-backed, not BFO-special-cased.

Minimum IR shape:

```rust
struct Ontology {
    entities: BTreeMap<TermId, Entity>,
    axioms: Vec<Axiom>,
}

struct Entity {
    id: TermId,
    kind: EntityKind,
    annotations: Annotations,
}

enum Axiom {
    SubClassOf { sub: ClassExpr, sup: ClassExpr },
    DisjointClasses { members: Vec<ClassExpr> },
    SubObjectPropertyOf { sub: ObjectPropertyExpr, sup: ObjectPropertyExpr },
    InverseObjectProperties { left: ObjectPropertyExpr, right: ObjectPropertyExpr },
    ObjectPropertyDomain { property: ObjectPropertyExpr, domain: ClassExpr },
    ObjectPropertyRange { property: ObjectPropertyExpr, range: ClassExpr },
    PropertyCharacteristic { property: TermId, characteristic: PropertyCharacteristic },
}

enum ClassExpr {
    Named(TermId),
    Union(Vec<ClassExpr>),
    Intersection(Vec<ClassExpr>),
    Complement(Box<ClassExpr>),
    SomeValuesFrom { property: ObjectPropertyExpr, filler: Box<ClassExpr> },
    AllValuesFrom { property: ObjectPropertyExpr, filler: Box<ClassExpr> },
}
```

Acceptance criteria:

- all later generation logic consumes the IR, never raw RDF triples
- the IR is explicitly shaped around eventual Rust code generation

## Phase 6: Lift BFO TTL Into The IR

Implement a one-way lowering from RDF graph to ontology IR.

Required lifting rules:

- `rdf:type owl:Class` -> class declaration
- `rdf:type owl:ObjectProperty` -> object property declaration
- `rdfs:label`, `skos:definition`, `skos:altLabel`, `skos:example`, `skos:scopeNote`, `dc11:identifier` -> annotations
- `rdfs:subClassOf` -> subclass axioms
- `owl:disjointWith` -> pairwise disjointness axioms
- `owl:AllDisjointClasses` + `owl:members` -> grouped disjointness axioms
- `rdfs:subPropertyOf` -> subproperty axioms
- `owl:inverseOf` -> inverse axioms
- `rdfs:domain` -> domain axioms
- `rdfs:range` -> range axioms
- `rdf:type owl:TransitiveProperty` -> property characteristic
- `rdf:type owl:FunctionalProperty` -> property characteristic
- `rdf:type owl:InverseFunctionalProperty` -> property characteristic

Required blank-node expression decoding:

- anonymous union
- anonymous intersection
- anonymous complement
- anonymous restriction with `owl:onProperty`
- `owl:someValuesFrom`
- `owl:allValuesFrom`

Hard rule:

- unsupported blank-node shapes are fatal build errors

Acceptance criteria:

- the entire current BFO core TTL file lifts successfully without OFN fallback

## Phase 7: Rebuild Analysis On Top Of The IR

Do not port the old helper maps directly. Re-derive generated data from the new axiom model.

Generate from IR:

- class inventory
- relation inventory
- canonical term metadata
- subclass parent edges
- relation parent edges
- class disjointness tables
- relation inverse tables
- domain and range expressions
- property flags
- preserved subclass restrictions

Refactor goal:

- generation code should read like ontology analysis, not parser post-processing
- analysis output should already be close to renderable Rust types and tables

Recommended analysis outputs:

- ordered `Vec<ClassDef>`
- ordered `Vec<RelationDef>`
- lookup indexes by:
  - IRI
  - OBO ID
  - spec ID
- typed semantic tables:
  - direct parent links
  - inverse links
  - disjoint sets
  - equivalent sets if supported later
  - domain and range expressions
  - subclass restriction payloads

Do not render Rust directly from raw axioms. Introduce a typed intermediate "generated model" that represents exactly what the Rust API needs.

Acceptance criteria:

- the renderer receives a clean analysis model with no knowledge of TTL blank nodes
- the renderer does not depend on `oxrdf` types

## Phase 7.5: Define The Rust Output Model Explicitly

Before rendering code, freeze the shape of the generated Rust-facing model.

Minimum generated model:

```rust
struct GeneratedOntology {
    classes: Vec<GeneratedClass>,
    relations: Vec<GeneratedRelation>,
}

struct GeneratedClass {
    id: String,
    iri: String,
    label: String,
    definition: Option<String>,
    spec_id: Option<String>,
    alt_labels: Vec<String>,
    examples: Vec<String>,
    scope_notes: Vec<String>,
    direct_parent_ids: Vec<String>,
    disjoint_ids: Vec<String>,
    subclass_constraints: Vec<GeneratedConstraint>,
    variant: String,
}

struct GeneratedRelation {
    id: String,
    iri: String,
    label: String,
    definition: Option<String>,
    spec_id: Option<String>,
    alt_labels: Vec<String>,
    examples: Vec<String>,
    scope_notes: Vec<String>,
    inverse_id: Option<String>,
    direct_parent_ids: Vec<String>,
    domain: Option<GeneratedClassExpr>,
    range: Option<GeneratedClassExpr>,
    flags: GeneratedRelationFlags,
    variant: String,
}
```

This model exists only to drive codegen cleanly. It is the final build-time representation before string rendering.

Acceptance criteria:

- codegen is a pure render step over a Rust-oriented typed model

## Phase 8: Replace The Test Suite Entirely

The current tests are contaminated by OFN assumptions.

Delete or rewrite:

- OFN line-count inventory checks
- OFN declaration scanners
- OFN-specific restriction counters

Replace them with:

- TTL-derived inventory checks
- IR completeness tests
- generated-model completeness tests
- RDF-list decoding tests
- blank-node expression decoding tests
- semantic table coherence tests
- generated Rust snapshot or structural tests

Required test categories:

- every declared class in TTL appears in generated output
- every declared object property in TTL appears in generated output
- every supported annotation family is preserved
- `owl:AllDisjointClasses` contributes to generated disjointness
- every supported property characteristic in TTL is reflected in generation
- domain/range expression decoding matches expected structures
- subclass restriction decoding matches expected structures
- generated symbol names are stable and collision-free
- lookup tables round-trip generated class and relation IDs correctly

Acceptance criteria:

- no test reads `bfo-core.ofn`
- no test depends on raw RDF parser internals when validating the public generated API

## Phase 9: Purge OFN Code And Artifacts From The Crate Surface

After TTL generation passes, remove OFN residue aggressively.

Delete:

- OFN parser helpers
- OFN-specific tests
- OFN parser comments
- OFN references in implementation docs

Potentially delete:

- the bundled OFN file from any crate-specific source manifest or assumptions if it is no longer needed locally

Keep only if there is a non-generator repository reason to retain it:

- the upstream vendored BFO distribution folder as a whole

Acceptance criteria:

- the `bfo` crate cannot accidentally regress back to OFN-driven parsing

## Phase 10: Tighten Failure Semantics

A destructive rewrite should make unsupported ontology structures impossible to ignore.

Add hard failures for:

- malformed RDF lists
- multiple conflicting labels where only one is expected
- restriction nodes missing `owl:onProperty`
- restriction nodes with unsupported fillers
- non-class expressions where a class expression is required
- non-object-property expressions where an object property is required
- unexpected BFO TTL shapes encountered during lifting

Acceptance criteria:

- the build fails early and specifically on unsupported source constructs

## Kill List

These concepts should disappear from the implementation:

- `parse_declarations(ofn, ...)`
- `parse_annotations(ofn)`
- `parse_subclass_axioms(ofn)`
- `parse_named_binary_axioms(ofn, ...)`
- `parse_named_group_axioms(ofn, ...)`
- `parse_inverses(ofn)`
- `parse_object_property_expr_map(ofn, ...)`
- `parse_property_flags(ofn, ...)`
- OFN filler strings as a first-class semantic representation
- tests that count lines starting with `Declaration(`

If any of these survive the rewrite, the rewrite is incomplete.

## Recommended Execution Order

1. Replace source paths and docs so OFN is no longer treated as canonical.
2. Stand up the RDF graph layer.
3. Implement RDF-list decoding.
4. Define the ontology IR.
5. Lift BFO TTL into the IR.
6. Rebuild semantic analysis on top of the IR.
7. Rewire the renderer to the new analysis output.
8. Replace the test suite.
9. Delete OFN code.
10. Delete any remaining OFN assumptions from docs and comments.

## Acceptance Criteria

This rewrite is complete when:

- `build.rs` reads only [bfo-core.ttl](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/bfo-core.ttl)
- there is no OFN parser in production code
- `oxrdfio` and `oxrdf` are the only RDF ingestion foundation
- the generated Rust API is produced entirely from TTL-derived IR and typed analysis output
- all tests validate TTL-derived semantics
- unsupported TTL/OWL structures fail loudly
- the implementation is simpler to extend toward other TTL-first ontologies
- no `oxrdf` or raw RDF term types leak into the public crate API

## First Execution Slice

The first serious slice should be:

1. rip out the OFN path from `main`
2. add `oxrdfio` and `oxrdf`
3. create RDF graph and list-decoding infrastructure
4. define the typed ontology IR and generated-model layer
5. get class and object-property inventory generation working from TTL only

That slice deliberately breaks the old implementation shape and establishes the irreversible direction of the rewrite.
