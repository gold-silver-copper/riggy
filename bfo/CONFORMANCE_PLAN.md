# BFO 2020 Conformance Plan

This document defines what it would take for the Rust `bfo` crate to be meaningfully conformant to the bundled BFO 2020 specification in [BFO-2020-master/README.md](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/README.md), [BFO-2020-master/21838-2/README.md](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/README.md), and the OWL / Common Logic artifacts under [BFO-2020-master/21838-2](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2).

## Scope

“Completely conformant” needs to be split into three layers:

1. `Vocabulary conformance`
   The crate exposes the complete BFO 2020 class and relation inventory, with canonical IDs, labels, IRIs, hierarchy, inverses, and domain/range metadata.

2. `Artifact conformance`
   The crate can reproduce or validate itself against the official BFO OWL core artifact and, optionally, the temporalized-relations profile.

3. `Axiom conformance`
   The crate encodes enough formal semantics to check the domain/range, inverse, symmetry, transitivity, and profile rules that are provable from the official artifacts. Full CLIF theorem proving is out of scope for ordinary runtime Rust code, but the crate should still validate itself against the official CLIF/OWL sources in tests.

The current crate is only a partial vocabulary sketch. It is not yet conformant at any of the three layers.

## Current Gap

The current [lib.rs](/Users/kisaczka/Desktop/code/riggy/bfo/src/lib.rs) mixes:

- a small subset of genuine BFO terms
- several game-specific relations
- a hand-maintained relation schema tuned to Riggy

That is useful for the game, but it is not publishable as a general BFO crate.

### Classes

The bundled `bfo-core.ofn` declares `36` classes. The current Rust crate models `18`.

Currently present:

- `Entity`
- `Continuant`
- `IndependentContinuant`
- `MaterialEntity`
- `Object`
- `ImmaterialEntity`
- `Site`
- `SpecificallyDependentContinuant`
- `Role`
- `Disposition`
- `Function`
- `Quality`
- `GenericallyDependentContinuant`
- `InformationContentEntity`
- `Occurrent`
- `Process`
- `History`
- `TemporalRegion`

Still missing from the BFO 2020 core hierarchy:

- `ObjectAggregate`
- `FiatObjectPart`
- `ContinuantFiatBoundary`
- `FiatPoint`
- `FiatLine`
- `FiatSurface`
- `SpatialRegion`
- `ZeroDimensionalSpatialRegion`
- `OneDimensionalSpatialRegion`
- `TwoDimensionalSpatialRegion`
- `ThreeDimensionalSpatialRegion`
- `RelationalQuality`
- `RealizableEntity`
- `ProcessBoundary`
- `SpatiotemporalRegion`
- `ZeroDimensionalTemporalRegion`
- `OneDimensionalTemporalRegion`
- `TemporalInstant`
- `TemporalInterval`

### Relations

The current crate exposes `10` relations, but several of them are not BFO relations at all.

Current Rust relations that are genuinely BFO-aligned:

- `SpecificallyDependsOn`
- `InheresIn`
- `HasParticipant`
- `OccursIn`

Current Rust relations that are too coarse or renamed beyond spec:

- `Occupies`
  BFO distinguishes `occupies spatial region`, `occupies temporal region`, and `occupies spatiotemporal region`.
- `ConnectedTo`
  Not a BFO core relation.

Current Rust relations that are Riggy-specific and must not live in the core `bfo` crate:

- `Contains`
- `ResidentOf`
- `IsAbout`
- `HasOutput`

Notes:

- `is about` is not a BFO core relation. It belongs in an extension layer, likely IAO-aligned, not in `bfo` core.
- `contains` and `resident of` are game/domain ontology relations and should move back into Riggy or a separate extension crate.

### Missing BFO relation families

The bundled relations table includes many relations that are not yet represented:

- `continuant part of` / `has continuant part`
- `proper continuant part of` / `has proper continuant part`
- `occurrent part of` / `has occurrent part`
- `proper occurrent part of` / `has proper occurrent part`
- `temporal part of` / `has temporal part`
- `proper temporal part of` / `has proper temporal part`
- `member part of` / `has member part`
- `located in` / `has location`
- `occupies spatial region`
- `occupies temporal region`
- `occupies spatiotemporal region`
- `bearer of`
- `realizes` / `has realization`
- `has material basis` / `material basis of`
- `generically depends on` / `is carrier of`
- `concretizes` / `is concretized by`
- `participates in`
- `has history` / `history of`
- `first instant of` / `has first instant`
- `last instant of` / `has last instant`
- `precedes` / `preceded by`
- `environs`
- `temporally projects onto`
- `spatially projects onto`
- `specifically depended on by`

## Source Of Truth

The Rust crate should stop being hand-authored term-by-term. The source of truth should be the bundled BFO artifacts:

- Core OWL:
  [BFO-2020-master/21838-2/owl/bfo-core.ofn](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/bfo-core.ofn)
- Core OWL RDF/XML:
  [BFO-2020-master/21838-2/owl/bfo-core.owl](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/bfo-core.owl)
- Core Common Logic modules:
  [BFO-2020-master/21838-2/common-logic](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/common-logic)
- Temporalized relations profile:
  [BFO-2020-master/21838-2/owl/profiles/temporal extensions/temporalized relations](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/profiles/temporal%20extensions/temporalized%20relations)
- Human-readable term inventories:
  [bfo-2020-terms.csv](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/profiles/temporal%20extensions/temporalized%20relations/documentation/bfo-2020-terms.csv)
  and
  [bfo-2020-relations-table.csv](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/owl/profiles/temporal%20extensions/temporalized%20relations/documentation/bfo-2020-relations-table.csv)

## Required Architectural Changes

### 1. Split `core BFO` from `application extensions`

The published `bfo` crate should contain only official BFO vocabulary and semantics.

Move out of `bfo` core:

- `Contains`
- `ResidentOf`
- `ConnectedTo`
- `IsAbout`
- `HasOutput`

Replacement plan:

- keep `bfo` strictly spec-derived
- add a Riggy-side extension enum for game relations
- if needed later, add companion crates such as `iao`, `ro`, or `bfo-ext`

### 2. Replace the current enums with a generated canonical term registry

The crate should define stable public Rust enums, but they should be generated from the official artifacts rather than manually curated.

Target shape:

```rust
pub enum BfoClass {
    Entity,
    Continuant,
    Occurrent,
    IndependentContinuant,
    SpecificallyDependentContinuant,
    GenericallyDependentContinuant,
    MaterialEntity,
    ImmaterialEntity,
    Site,
    SpatialRegion,
    ContinuantFiatBoundary,
    FiatPoint,
    FiatLine,
    FiatSurface,
    Object,
    ObjectAggregate,
    FiatObjectPart,
    Quality,
    RelationalQuality,
    RealizableEntity,
    Role,
    Disposition,
    Function,
    InformationContentEntity,
    Process,
    ProcessBoundary,
    History,
    SpatiotemporalRegion,
    TemporalRegion,
    ZeroDimensionalSpatialRegion,
    OneDimensionalSpatialRegion,
    TwoDimensionalSpatialRegion,
    ThreeDimensionalSpatialRegion,
    ZeroDimensionalTemporalRegion,
    OneDimensionalTemporalRegion,
    TemporalInstant,
    TemporalInterval,
}
```

And similarly for relations:

```rust
pub enum BfoRelation {
    ContinuantPartOf,
    HasContinuantPart,
    ProperContinuantPartOf,
    HasProperContinuantPart,
    OccurrentPartOf,
    HasOccurrentPart,
    ProperOccurrentPartOf,
    HasProperOccurrentPart,
    TemporalPartOf,
    HasTemporalPart,
    ProperTemporalPartOf,
    HasProperTemporalPart,
    MemberPartOf,
    HasMemberPart,
    SpecificallyDependsOn,
    SpecificallyDependedOnBy,
    GenericallyDependsOn,
    IsCarrierOf,
    InheresIn,
    BearerOf,
    Realizes,
    HasRealization,
    HasMaterialBasis,
    MaterialBasisOf,
    Concretizes,
    IsConcretizedBy,
    LocatedIn,
    HasLocation,
    OccupiesSpatialRegion,
    OccupiesTemporalRegion,
    OccupiesSpatiotemporalRegion,
    OccursIn,
    Environs,
    HasParticipant,
    ParticipatesIn,
    HasHistory,
    HistoryOf,
    FirstInstantOf,
    HasFirstInstant,
    LastInstantOf,
    HasLastInstant,
    Precedes,
    PrecededBy,
    TemporallyProjectsOnto,
    SpatiallyProjectsOnto,
}
```

### 3. Add canonical metadata for every term

Each term should carry:

- BFO ID, for example `BFO:0000001`
- canonical IRI
- preferred label
- optional textual definition
- parent term
- inverse relation, if any
- domain and range restrictions
- source profile: `core` or `temporalized_relations`

Target API:

```rust
pub struct TermMeta {
    pub id: &'static str,
    pub iri: &'static str,
    pub label: &'static str,
    pub definition: Option<&'static str>,
}
```

And:

```rust
pub struct RelationMeta {
    pub id: &'static str,
    pub iri: &'static str,
    pub label: &'static str,
    pub inverse: Option<BfoRelation>,
    pub domain: &'static [BfoClass],
    pub range: &'static [BfoClass],
    pub profile: Profile,
}
```

### 4. Model profiles explicitly

The bundled spec distinguishes the core OWL artifact from stronger temporalized-relations material. The Rust crate should expose that distinction directly.

```rust
pub enum Profile {
    CoreOwl,
    TemporalizedRelations,
    CommonLogicReference,
}
```

This matters because “complete conformance” is ambiguous unless the crate states which profile a term belongs to.

### 5. Move from ad hoc validation to BFO semantics

The current `RelationSpec` is too thin. For conformance, the crate needs to distinguish:

- inverse
- reverse
- temporalized variants
- profile membership
- whether a relation is transitive
- whether it is reflexive or irreflexive
- whether it is asymmetric or symmetric

Target:

```rust
pub struct RelationSemantics {
    pub inverse: Option<BfoRelation>,
    pub transitive: bool,
    pub symmetric: bool,
    pub asymmetric: bool,
    pub reflexive_on: Option<&'static [BfoClass]>,
    pub profile: Profile,
}
```

Not every logical property can be reconstructed from OWL alone, so some of this metadata will need to be sourced from the Common Logic modules and maintained in generated data.

## Conformance Roadmap

### Phase 1: Make the crate vocabulary-correct

Deliverables:

- all BFO 2020 classes in Rust
- all core and temporalized relation names in Rust
- canonical IDs, labels, IRIs
- parent/child hierarchy
- inverse mappings
- profile tagging

Acceptance criteria:

- every class declared in `bfo-core.ofn` exists in Rust
- every object property declared in `bfo-core.ofn` exists in Rust
- every temporalized-relation term in the bundled documentation is either represented or explicitly marked unsupported
- zero Riggy-specific terms remain in the public core API

### Phase 2: Make the crate metadata-correct

Deliverables:

- generated `TermMeta` and `RelationMeta`
- domain/range metadata
- definitions and labels imported from the spec artifacts or documentation CSVs

Acceptance criteria:

- tests compare all Rust term IDs/labels against the bundled CSV and OWL data
- tests verify inverse mappings and parent chains

### Phase 3: Make the crate semantically-correct for runtime validation

Deliverables:

- a richer `RelationSemantics` layer
- validated domain/range checks
- validation rules for inverse, partonomy, and temporal-region constraints where the artifacts support them

Acceptance criteria:

- relation-domain/range tests pass against the official inventory
- inverse pairs are complete and internally consistent
- profile-specific rules are enforced only in the profiles where they belong

### Phase 4: Add artifact validation and export

Deliverables:

- test tooling that reads the bundled OWL functional syntax and checks Rust registry parity
- optional export of Rust registry back into OWL Functional Syntax or JSON

Acceptance criteria:

- a test can assert exact parity for declared classes and object properties
- a test can assert exact parity for labels and IDs

### Phase 5: Add stronger logic checks against the CLIF modules

Deliverables:

- a curated set of test assertions derived from:
  [common-logic](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/21838-2/common-logic)
  and
  [documentation/axiomatization-pds](/Users/kisaczka/Desktop/code/riggy/bfo/BFO-2020-master/documentation/axiomatization-pds)
- encoded invariants for relations such as temporal-part and participation constraints

Acceptance criteria:

- crate tests cover the major subtheories:
  material entity
  continuant mereology
  occurrent mereology
  specific dependence
  generic dependence
  participation
  temporal region
  spatiotemporal region
  history

This still will not turn Rust into a Common Logic theorem prover, but it will make the crate demonstrably traceable to the formal source artifacts.

## Recommended Crate Layout

Recommended target structure:

```text
bfo/
  src/
    lib.rs
    class.rs
    relation.rs
    profile.rs
    metadata.rs
    registry.rs
    validate.rs
    generated/
      classes.rs
      relations.rs
      metadata.rs
  tools/
    extract_bfo_terms.rs
    check_owl_parity.rs
  BFO-2020-master/
  CONFORMANCE_PLAN.md
```

Key rule:

- `generated/` should be derived from the bundled official artifacts
- handwritten code should only define API shape, validation logic, and generation tooling

## Recommended Non-Goals

These should not be part of the first conformance pass:

- implementing a full Common Logic reasoner in Rust
- merging BFO with IAO/RO/OBI in the same crate
- keeping Riggy-specific relation names in the core ontology crate
- treating the current game ontology as if it were identical to BFO

## Immediate Next Steps

1. Rename the current public relation enum to `BfoRelation` and remove non-BFO relations from it.
2. Add the missing BFO class variants and explicit BFO IDs.
3. Add a `Profile` enum and mark which relations belong to `core` versus `temporalized_relations`.
4. Introduce generated metadata tables instead of handwritten labels.
5. Add a parity test that counts and matches classes and object properties against `bfo-core.ofn`.
6. Add a Riggy-side extension relation enum for `Contains`, `ResidentOf`, `IsAbout`, `HasOutput`, and any future game-only relations.

## Definition Of Done

This crate can be called “BFO 2020 conformant” when all of the following are true:

- it exposes the full BFO 2020 term inventory from the bundled artifacts
- it does not mix game/domain-specific relations into the core ontology vocabulary
- every public term has canonical ID, IRI, label, hierarchy, and inverse metadata where applicable
- tests prove parity against the bundled OWL core artifact
- tests cover a meaningful subset of the stronger CLIF axioms as executable conformance checks

Until then, the crate should be described as:

`a BFO-inspired Rust ontology vocabulary`, not `a conformant BFO 2020 implementation`
