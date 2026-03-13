# BFO Generator Implementation Plan

This document defines the next implementation steps for the `bfo` crate generator in [build.rs](/Users/kisaczka/Desktop/code/riggy/bfo/build.rs).

The goal is to move from a narrow enum generator to a source-driven ontology registry that derives its public API and semantic tables directly from the bundled BFO artifacts, with minimal hardcoding.

## Progress Notes

Updated March 13, 2026.

Implemented so far:

- generated lookup APIs for classes and relations by OBO ID, IRI, and spec ID
- generated direct term accessors for `spec_id`, `alt_labels`, `examples`, and `scope_notes`
- generated class disjointness tables from `DisjointClasses`
- generated relation parent tables from `SubObjectPropertyOf`
- preserved restriction-bearing `SubClassOf(...)` axioms as generated `ClassConstraint` values
- added tests covering lookup round-trips, annotation preservation, disjointness, subproperty parents, and subclass restriction counts

Current state of the generator:

- still reads only `bfo-core.ofn`
- still uses line-oriented parsing instead of a full structured OFN parser
- still derives public Rust variants from normalized labels rather than canonical IDs
- now preserves top-level quantified subclass restrictions, but exposes fillers as OFN strings rather than fully typed generated expression values
- still does not model a generic ontology IR shared by all axiom families

Current test status:

- `cargo test` in `/Users/kisaczka/Desktop/code/riggy/bfo` passes with 9 tests

## Objectives

- Generate lookup APIs and complete term accessors, not just enums.
- Parse and emit more of the ontology's formal semantics.
- Eliminate label-derived and artifact-specific assumptions where possible.
- Fail loudly on unsupported ontology syntax instead of silently dropping semantics.
- Keep the source of truth in the bundled BFO artifacts, not hand-maintained Rust tables.

## Current Constraints

The current generator:

- reads only `BFO-2020-master/21838-2/owl/bfo-core.ofn`
- uses line-oriented string matching instead of a real OFN parser
- generates only `BfoClass` and `RelationKind`
- derives Rust variants from labels
- captures more annotations and property axioms than before, but not all ontology structure
- preserves top-level quantified subclass restrictions, but not yet as fully typed generated expression trees

This is sufficient for a typed vocabulary sketch, but not for complete BFO integration.

## Target Outcome

The generated crate should behave as a small ontology registry with:

- typed enums for classes and object properties
- stable generated term data for every term
- generated lookup APIs by canonical identifiers
- generated axiom tables for class and property semantics
- tests that prove source completeness against the selected BFO artifacts

## Public API To Generate

### Class API

Generate:

- `BfoClass::ALL`
- `BfoClass::from_obo_id(&str) -> Option<Self>`
- `BfoClass::from_iri(&str) -> Option<Self>`
- `BfoClass::from_spec_id(&str) -> Option<Self>`
- `BfoClass::spec_id(self) -> Option<&'static str>`
- `BfoClass::alt_labels(self) -> &'static [&'static str]`
- `BfoClass::examples(self) -> &'static [&'static str]`
- `BfoClass::scope_notes(self) -> &'static [&'static str]`
- `BfoClass::direct_parents(self) -> &'static [BfoClass]`
- `BfoClass::disjoint_with(self) -> &'static [BfoClass]`
- `BfoClass::equivalent_to(self) -> &'static [BfoClass]`
- `BfoClass::subclass_constraints(self) -> &'static [ClassConstraint]`

Optional later:

- `BfoClass::from_label(&str) -> Option<Self>`
- `BfoClass::from_alt_label(&str) -> Option<Self>`

### Relation API

Generate:

- `RelationKind::ALL`
- `RelationKind::from_obo_id(&str) -> Option<Self>`
- `RelationKind::from_iri(&str) -> Option<Self>`
- `RelationKind::from_spec_id(&str) -> Option<Self>`
- `RelationKind::spec_id(self) -> Option<&'static str>`
- `RelationKind::alt_labels(self) -> &'static [&'static str]`
- `RelationKind::examples(self) -> &'static [&'static str]`
- `RelationKind::scope_notes(self) -> &'static [&'static str]`
- `RelationKind::direct_parents(self) -> &'static [RelationKind]`
- `RelationKind::equivalent_to(self) -> &'static [RelationKind]`
- `RelationKind::disjoint_with(self) -> &'static [RelationKind]`

Optional later:

- `RelationKind::from_label(&str) -> Option<Self>`
- `RelationKind::from_alt_label(&str) -> Option<Self>`

### Generated Data Tables

The generator will still need stable internal tables, but those should remain a codegen detail rather than part of the public API.

Implementation preference:

- keep the public surface enum-centric
- generate private static tables keyed by enum discriminant
- implement public enum methods as direct accessors over those tables

## Source Model Changes

The generator should stop treating the ontology as a bag of special-case maps. Replace the current `ClassDef` and `RelationDef`-centric model with a generic ontology IR.

Recommended internal shape:

```rust
struct Ontology {
    classes: BTreeMap<TermId, ClassTerm>,
    object_properties: BTreeMap<TermId, ObjectPropertyTerm>,
    class_axioms: Vec<ClassAxiom>,
    property_axioms: Vec<PropertyAxiom>,
}

struct ClassTerm {
    id: TermId,
    annotations: Annotations,
}

struct ObjectPropertyTerm {
    id: TermId,
    annotations: Annotations,
}

struct TermId {
    obo_id: String,
    iri: String,
}

struct Annotations {
    label: Option<String>,
    definition: Option<String>,
    alt_labels: Vec<String>,
    examples: Vec<String>,
    scope_notes: Vec<String>,
    spec_id: Option<String>,
}
```

Class and property semantics should live in axiom enums, not as hardcoded fields attached only to the handful of semantics currently supported.

## Parser Work

## Phase 1: Replace Line-Oriented Parsing With Structured OFN Parsing

Status:

- `in progress`
- nested class-expression parsing now handles `ObjectAllValuesFrom` and `ObjectSomeValuesFrom`
- top-level parsing is still line-oriented and should still be replaced

Implement a minimal parser for the OFN forms that appear in bundled BFO artifacts.

Required parser features:

- balanced parsing of nested forms
- named class expressions
- named object properties
- annotation assertions
- top-level axioms with typed variants

Required class expression support:

- named classes
- `ObjectUnionOf`
- `ObjectIntersectionOf`
- `ObjectComplementOf`
- `ObjectAllValuesFrom`
- `ObjectSomeValuesFrom`

Optional later:

- cardinality restrictions if future artifacts require them

The parser should return an explicit error on any unsupported top-level axiom or nested expression seen in the selected source set.

## Phase 2: Parse Complete Term Data

Status:

- `partially complete for current core source set`
- implemented: `rdfs:label`, `skos:definition`, `skos:altLabel`, `skos:example`, `skos:scopeNote`, `dc11:identifier`
- not yet implemented as generated lookups: label and alt-label reverse indexes

Parse and store:

- `rdfs:label`
- `skos:definition`
- `skos:altLabel`
- `skos:example`
- `skos:scopeNote`
- `dc11:identifier`

This removes the current annotation loss and allows generated lookup tables and direct enum accessors beyond enum names.

## Phase 3: Parse Class Axioms

Status:

- `partially complete`
- implemented: named `SubClassOf`, `DisjointClasses`, top-level `ObjectAllValuesFrom` / `ObjectSomeValuesFrom` preservation inside `SubClassOf`
- not yet implemented: generic typed preservation for arbitrary superclass expressions, `EquivalentClasses` generation, or a shared class-axiom IR

Add support for:

- `SubClassOf`
- `EquivalentClasses`
- `DisjointClasses`

Model subclass axioms in full, including restriction-bearing forms. Do not drop `SubClassOf(...)` axioms just because they contain `Object...`.

Important distinction:

- simple named superclass edges should feed hierarchy helpers
- restriction-bearing subclass axioms should be preserved as generated semantic data

Current implementation note:

- restriction-bearing subclass axioms are currently exposed as `ClassConstraint` values with a typed relation and an OFN filler string
- this is a useful intermediate step, but not the final typed expression model

## Phase 4: Parse Property Axioms

Status:

- `partially complete`
- implemented: `ObjectPropertyDomain`, `ObjectPropertyRange`, `InverseObjectProperties`, `SubObjectPropertyOf`, `DisjointObjectProperties`, `EquivalentObjectProperties`, and all current property-characteristic flags at parser level
- generated semantic accessors currently exist for domain/range, inverse, direct parents, disjointness, equivalence, and characteristic flags
- current `bfo-core.ofn` does not contain `EquivalentObjectProperties`, `DisjointObjectProperties`, `AsymmetricObjectProperty`, `ReflexiveObjectProperty`, or `IrreflexiveObjectProperty`, but parser support exists

Add support for:

- `ObjectPropertyDomain`
- `ObjectPropertyRange`
- `InverseObjectProperties`
- `SubObjectPropertyOf`
- `EquivalentObjectProperties`
- `DisjointObjectProperties`
- `SymmetricObjectProperty`
- `TransitiveObjectProperty`
- `FunctionalObjectProperty`
- `InverseFunctionalObjectProperty`
- `AsymmetricObjectProperty`
- `ReflexiveObjectProperty`
- `IrreflexiveObjectProperty`

Even if some of these are absent in current `bfo-core.ofn`, parser support should still exist so the generator is artifact-driven rather than tailored to one snapshot.

## Code Generation Work

## Phase 5: Generate Stable Symbols

Status:

- `partially complete`
- normalization is more robust than before and no longer generates obviously invalid fallback identifiers
- public variants are still label-derived, so this phase is not done

Stop deriving enum variant names directly from labels alone.

Requirements:

- choose canonical Rust symbol names from stable ontology identifiers
- preserve labels purely as term data
- detect collisions deterministically
- emit a build error if two terms would map to the same public symbol

If human-friendly label-based symbols remain desirable, generate them from canonical IDs with an override-free normalization rule and collision checks.

## Phase 6: Generate Internal Term Data Tables

Status:

- `partially complete`
- generation is still done through match arms instead of centralized internal tables
- public enum methods now cover substantially more term data than before

Generate:

- static arrays for all classes and relations
- lookup tables by OBO ID and IRI
- optional lookup tables by spec identifier and labels

Implementation note:

- enum methods like `label()`, `definition()`, `iri()`, `parent()`, and `inverse()` should read from generated internal tables where practical

This keeps the generated surface flat while still making the generator easier to extend.

## Phase 7: Generate Semantic Tables

Status:

- `partially complete`
- implemented: class direct-parent edges, class disjointness, relation direct-parent edges, relation inverse links, relation domain/range expressions, relation characteristic flags, preserved quantified subclass constraints
- not yet implemented: fully typed class equivalence tables, fully typed property equivalence/disjointness tables, or generic axiom registries

Generate reusable semantic data instead of only boolean helpers:

- class direct-parent edges
- class disjointness sets
- class equivalence sets
- relation direct-parent edges
- relation inverse links
- relation equivalence sets
- relation disjointness sets
- relation domain/range expressions
- relation characteristic flags
- preserved restriction axioms

This should support both convenience helpers and downstream reasoning code.

## Artifact Strategy

## Phase 8: Support More Than One BFO Artifact

Move from a hardcoded single-file input to an explicit source manifest inside `build.rs`.

Initial source sets:

- `core`: `BFO-2020-master/21838-2/owl/bfo-core.ofn`
- optional future `temporalized_relations` profile files under `21838-2/owl/profiles/temporal extensions/...`

Requirements:

- emit `cargo:rerun-if-changed` for every selected source file
- isolate generated modules by source set if needed
- make profile support additive, not hand-merged

## Validation And Tests

## Phase 9: Add Completeness Tests

Tests should verify that generation covers the ontology source, not just a few hand-picked examples.

Add tests for:

- generated class inventory matches declarations
- generated relation inventory matches declarations
- every declared term has generated term data
- every parsed annotation family is reflected in generated accessors or internal tables
- every supported axiom family in the source appears in generated tables
- unsupported syntax causes a build failure rather than silent omission

## Phase 10: Add Semantic Consistency Tests

Add tests that confirm generated semantics are coherent:

- inverse links are symmetric
- subproperty ancestry is acyclic
- disjointness tables are symmetric
- equivalence tables are symmetric
- hierarchy helpers agree with generated subclass edges
- generated domain/range predicates match preserved expressions

## Suggested Execution Order

1. Introduce generic ontology IR types in `build.rs`.
2. Replace current annotation parsing with reusable annotation collection.
3. Implement structured OFN parsing for the axiom families already present in `bfo-core.ofn`.
4. Generate `from_obo_id` / `from_iri` lookup APIs and direct enum accessors for parsed term data.
5. Refactor existing enum methods to delegate to generated internal tables.
6. Add generated support for `SubObjectPropertyOf` and `DisjointClasses`.
7. Preserve restriction-bearing `SubClassOf` axioms instead of dropping them.
8. Add parser support for the remaining ontology axiom families even if current core data does not use all of them.
9. Add source-set support for future profile generation.
10. Expand tests from inventory checks to full completeness checks.

Progress against this order:

- completed: 2, 4, 6
- partially completed: 3, 5, 7, 10
- current next step: 1 or a lighter-weight precursor to 1 by introducing a shared ontology IR for parsed class and property axioms before replacing the full parser

## Acceptance Criteria

This implementation is successful when:

- no term inventory is hand-maintained
- no generated symbol depends on ad hoc manual mapping
- lookup by OBO ID and IRI is generated for classes and relations
- lookup by spec ID is generated for classes and relations
- term data is preserved from source annotations
- property hierarchy and disjointness data are generated from source axioms
- restriction-bearing class axioms are preserved instead of discarded
- the build fails on unsupported ontology syntax in selected sources
- tests prove completeness against the selected BFO source set

## Non-Goals For The First Pass

- full Common Logic theorem proving
- runtime OWL reasoning engine behavior
- compatibility shims for older hand-authored `bfo` APIs
- importing unrelated ontology families into `bfo` core

The first pass should focus on correct source-driven generation from bundled BFO OWL artifacts.
