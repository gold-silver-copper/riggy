# Architecture Roadmap

## Goal

Move the game from a prototype architecture to a production-grade, type-safe system for AI-driven gameplay.

The main constraints for this roadmap are:

- no string-matching control flow
- no UI copy embedded in domain logic
- no direct LLM-to-world mutation path
- graph state remains authoritative
- AI context, proposals, and validation are explicit and testable

## Current Status

Completed phases:

- [x] Phase 1: typed read-model and presenter boundary
- [x] Phase 2: typed command/event application boundary
- [x] Phase 3: graph-authoritative identity and invariants
- [x] Phase 4: canonical semantic vocabularies

Not started:

- [ ] Phase 5: AI proposal/validation boundary
- [ ] Phase 6: versioned AI context contracts
- [ ] Phase 7: structured relationship memory
- [ ] Phase 8: persistence versioning
- [ ] Phase 9: TUI runtime state machine hardening

What is true in the code now:

- `src/tui.rs` consumes typed `UiSnapshot` read models from `GameService::snapshot()`
- `src/presenter.rs` owns world prose and event notice rendering
- `src/domain/commands.rs` and `src/domain/events.rs` define the typed application boundary
- `src/app/service.rs` owns command handling and emits typed `GameEvent`s
- `src/app/read_model.rs` owns `UiSnapshot` projection from `GameState`
- `src/app/query.rs` owns shared query helpers used by both service validation and read-model projection
- `src/world.rs` uses the graph as the authoritative source of node identity and containment
- `src/domain/invariants.rs` validates graph structure and cross-edge consistency
- `src/domain/vocab.rs` owns canonical enums for city and NPC semantics
- `GameService::new` and `GameService::load` validate worlds before gameplay begins
- world generation now samples typed semantic vocabularies rather than raw string tables
- `DialogueRequest` now carries canonical typed semantic fields, with prompt rendering responsible for human-readable labels

## Current Problems

### 1. The AI boundary is still too weak

The LLM still returns `WorldAction`, and the application service still applies those actions directly after generation. The action set is small, but the architectural shape is still "model proposes mutation, service applies it" rather than "model proposes, validator checks, policy approves, application translates".

### 2. AI context and prompt construction are still ad hoc

Dialogue context is assembled directly from live game state into prompt-facing structs, and prompt construction is still string-heavy. The system does not yet have versioned AI contracts or a clean separation between authoritative facts, derived facts, and presentation text.

### 3. Relationship memory is still too freeform

`RelationshipState.memory_summary` is still a plain `String`. That is enough for current behavior, but it is not yet machine-usable memory the game can reason over.

### 4. Persistence is not versioned

Save/load works, but persisted state is not schema-versioned yet. That makes future refactors riskier than they need to be.

### 5. The TUI runtime still relies on an ad hoc UI state machine

The command boundary is typed, but modal behavior in `src/tui.rs` is still encoded directly in event-handler branches rather than a dedicated typed UI state machine.

## Target Architecture

Split the codebase into four layers.

### Domain

Owns:

- ids
- graph-backed world state
- canonical enums and value types
- invariants
- domain commands and domain events

Must not own:

- UI text
- prompt strings
- provider-specific AI logic

### Application

Owns:

- orchestration
- command handling
- validation
- service coordination
- state transitions

Must expose:

- typed commands in
- typed events and read models out

### AI

Owns:

- dialogue context building
- prompt construction
- provider adapters
- structured proposal extraction
- proposal validation

Must not:

- mutate game state directly

### UI

Owns:

- keybindings
- menus
- input modes
- rendering

Must consume:

- typed read models only

## Implementation Phases

### Phase 1: Extract Typed Read Models

Goal:

Remove presentation formatting from simulation.

Progress:

- [x] renamed `cli` module to `tui`
- [x] replaced `UiSnapshot` string bags like `known_info`, `people`, `cars`, and `things`
- [x] added typed read-model structs for status, city, place, actors, entities, routes, and context feed
- [x] moved world text and menu label formatting into a presenter module
- [x] updated the TUI runtime to render from the presenter
- [x] removed remaining duplicated display-oriented fields from the read model where nested typed views were sufficient
- [x] added presenter-focused tests that do not instantiate the full game runtime

Status:

- Phase 1 is complete.
- Remaining UI cleanup now belongs to later phases, especially the command/event split in Phase 2 and deeper domain normalization in Phase 3 and Phase 4.

Work:

- replace `UiSnapshot` with typed view structs
- remove `known_info: Vec<String>`, `people: Vec<String>`, `things: Vec<String>`, and other display-oriented bags
- add view types such as:
  - `PlaceView`
  - `RouteView`
  - `ActorView`
  - `EntityView`
  - `ContextFeedEntryView`
- move all prose and label formatting into a new presenter module

Deliverables:

- `src/presenter.rs`
- typed read-model structs
- `tui.rs` renders from typed data only

Acceptance criteria:

- simulation no longer formats user-facing strings
- TUI no longer strips prefixes like `Time: `
- UI rendering tests can be written without instantiating `Game`

### Phase 2: Introduce Commands and Events

Goal:

Stop treating `Game` as a monolithic service.

Progress:

- [x] replaced string-based command return values with typed `CommandResult`
- [x] introduced typed application events for travel, dialogue lifecycle, vehicle entry/exit, inspection, waiting, and relationship updates
- [x] updated the TUI to render notices from typed events through the presenter
- [x] split commands and events into dedicated modules outside `simulation.rs`
- [x] replaced the monolithic `simulation::Game` service with `app::service::GameService`
- [x] made system context feed entries structured state instead of service-formatted UI strings
- [x] removed the remaining string-returning helper methods from the application layer

Status:

- Phase 2 is complete.
- The application boundary now consists of typed commands, typed events, and a dedicated `GameService`.
- `simulation.rs` is now state/read-model focused, while the TUI and presenter consume the command/event boundary instead of calling string-based action paths.

Work:

- add typed application commands:
  - `StartDialogue`
  - `SubmitDialogueLine`
  - `LeaveDialogue`
  - `Travel`
  - `EnterVehicle`
  - `ExitVehicle`
  - `InspectEntity`
  - `Wait`
- add typed domain/application events:
  - `DialogueStarted`
  - `DialogueLineRecorded`
  - `TravelCompleted`
  - `VehicleEntered`
  - `VehicleExited`
  - `RelationshipChanged`
  - `ContextFeedAppended`
- replace `CommandOutput { text, should_quit }` with typed results

Deliverables:

- `src/domain/commands.rs`
- `src/domain/events.rs`
- `src/app/service.rs`

Acceptance criteria:

- CLI dispatches typed commands only
- state transitions emit typed events
- command handlers contain validation and no UI prose

Completion notes:

- `src/domain/commands.rs` now owns the gameplay command surface.
- `src/domain/events.rs` now owns the application event/result surface.
- `src/app/service.rs` is now the authoritative application service boundary.
- `src/app/read_model.rs` now owns `UiSnapshot` projection from `GameState`.
- `src/app/query.rs` now owns shared state query helpers used by both command validation and read-model projection.
- `src/simulation.rs` now contains state and typed read models only.
- system context feed entries were converted from `{ label, text }` strings into typed variants rendered by `src/presenter.rs`.
- NPC reply submission now goes through `GameCommand::SubmitDialogueLine` rather than a side-channel service method.
- `GameEvent` payloads now use dedicated domain refs/value types instead of `simulation` view structs.
- dialogue and system feed mutations now emit typed events (`DialogueLineRecorded` / `ContextAppended`) instead of being invisible side effects.
- the TUI now consumes `GameService::snapshot()` instead of accessing raw `GameState`.
- service/query read paths were deduplicated through `src/app/query.rs`.

### Phase 3: Make the Graph Truly Authoritative

Goal:

Remove duplicated identity and relationship assumptions.

Progress:

- [x] removed duplicated id fields from `City`, `Place`, `Npc`, and `Entity`
- [x] removed `Place.city_id` and made city containment edge-derived
- [x] removed node-id registry vectors from `World` and derived ids from the graph itself
- [x] stopped construction from using placeholder ids followed by mutation
- [x] added explicit graph invariant validation in `src/domain/invariants.rs`
- [x] added regression coverage for invalid graph states
- [x] added cross-edge validation for NPC resident city vs present place city consistency
- [x] enforced world validation in `GameService::new` and `GameService::load`

Status:

- Phase 3 is complete.
- The graph is now the authoritative source of identity and containment, and world validation is explicit instead of implicit.

Work:

- stop storing ids redundantly inside node payloads unless absolutely required
- either:
  - remove `id` fields from `City`, `Place`, `Npc`, and `Entity`, or
  - centralize them in typed record wrappers
- audit denormalized fields like `Place.city_id`
- formalize containment and residency rules in an invariant layer

Deliverables:

- `src/domain/invariants.rs`
- graph validation API

Acceptance criteria:

- world can be validated explicitly
- invalid graph states are detectable in tests
- construction code no longer relies on placeholder ids followed by mutation

Completion notes:

- `World` now stores only `seed` and `graph`; node ids are derived from graph node indices on demand.
- `City`, `Place`, `Npc`, and `Entity` no longer mirror graph identity inside payload structs.
- `Place` no longer stores `city_id`; `World::place_city_id` resolves that from `ContainsPlace` edges.
- `src/domain/invariants.rs` now validates containment, residency, route endpoint rules, and NPC resident-city vs present-place-city consistency.
- `World::validate()` provides an explicit graph validation API for runtime/debug/test use.
- `GameService::new` now rejects invalid generated worlds before runtime starts.
- `GameService::load` now rejects invalid saved worlds before they can enter gameplay.
- regression coverage exists for both graph-layer invalid states and invalid load-time snapshots.

### Phase 4: Replace Raw Semantic Strings With Canonical Types

Goal:

Turn world semantics into typed vocabularies.

Progress:

- [x] added canonical enums in `src/domain/vocab.rs` for `Biome`, `Economy`, `Culture`, `NpcArchetype`, `Occupation`, `TraitTag`, and `GoalTag`
- [x] updated `src/world.rs` to store typed semantics in `City` and `Npc`
- [x] updated procgen to sample canonical vocab values directly rather than generating semantic strings first
- [x] updated typed read models to carry canonical semantic values into the presenter
- [x] updated `DialogueRequest` to carry canonical typed semantic fields
- [x] moved human-readable rendering of those semantics to presenter/prompt code through `.label()` methods

Status:

- Phase 4 is complete.
- Core world semantics now exist as canonical types in domain state, read models, and AI request contracts.

Work:

- add enums/newtypes for:
  - `Biome`
  - `Economy`
  - `Culture`
  - `NpcArchetype`
  - `Occupation`
  - `TraitTag`
  - `GoalTag`
- distinguish canonical tags from flavor text
- update procgen to generate typed tags first, then human-readable strings second

Deliverables:

- `src/domain/vocab.rs`
- updated world generation and AI context generation

Acceptance criteria:

- domain rules use canonical types instead of matching strings
- prompts are built from typed fields
- future simulation systems can branch on enums, not prose

Completion notes:

- `src/domain/vocab.rs` now owns the canonical semantic vocabulary for the world layer.
- `City.biome`, `City.economy`, and `City.culture` are now typed enums instead of `String`.
- `Npc.archetype`, `Npc.occupation`, `Npc.personality_traits`, and `Npc.goal` are now typed enums/tags instead of `String`.
- `src/app/read_model.rs` now projects those canonical types directly into `UiSnapshot`.
- `src/presenter.rs` now renders semantic labels from canonical types instead of receiving preformatted semantics.
- `src/llm.rs` now builds prompts from typed semantic fields in `DialogueRequest`.
- procgen still produces freeform flavor text such as district descriptions, place descriptions, names, and landmarks, but canonical simulation semantics are now separate from that flavor text.

### Phase 5: Redesign the AI Boundary Around Proposals

Goal:

Ensure the LLM cannot directly mutate authoritative state.

Work:

- replace `WorldAction` with `AiProposal`
- add proposal categories such as:
  - `RelationshipAdjustmentProposal`
  - `MemoryUpdateProposal`
  - `NoChange`
- add a validator/policy layer:
  - bounds checks
  - target checks
  - session checks
  - trust policy checks
- convert validated proposals into commands or events

Deliverables:

- `src/ai/proposals.rs`
- `src/ai/validation.rs`
- `src/ai/policy.rs`

Acceptance criteria:

- no LLM output is applied directly to state
- invalid proposals are rejected safely
- accepted proposals are auditable and testable

### Phase 6: Version AI Context and Prompt Contracts

Goal:

Make AI context explicit, stable, and testable.

Work:

- add typed context objects:
  - `NpcDialogueContextV1`
  - `DialogueTurnContextV1`
  - `RelationshipMemoryViewV1`
- add a prompt builder that consumes only those contracts
- version the contracts explicitly so prompt changes are intentional
- separate authoritative facts from presentation text

Deliverables:

- `src/ai/context.rs`
- `src/ai/prompting.rs`

Acceptance criteria:

- AI requests are built from versioned context structs
- prompt tests use context fixtures instead of live game state
- changing prompt shape does not require touching simulation logic

### Phase 7: Replace Freeform Memory With Structured Memory

Goal:

Store durable conversation state in a form the game can reason about.

Work:

- replace plain `memory_summary: String` with a structured memory object
- proposed shape:
  - `trust_delta_summary`
  - `known_topics`
  - `unresolved_threads`
  - `freeform_summary`
- make the summarizer produce typed updates where possible

Deliverables:

- `src/domain/relationship.rs`
- updated summarization path

Acceptance criteria:

- relationship state contains machine-usable memory
- AI context can include both structured and freeform memory

### Phase 8: Introduce Persistence Versioning

Goal:

Make saves resilient to schema changes.

Work:

- add a save schema version
- separate persisted state from runtime-only UI/application state
- add migrations where needed

Deliverables:

- `src/persistence/mod.rs`
- versioned save schema

Acceptance criteria:

- saves can be migrated intentionally
- UI-only state is not persisted accidentally

### Phase 9: Harden the TUI Runtime

Goal:

Make UI state machine logic explicit and testable.

Work:

- replace coarse `Mode`/`Menu` handling with a typed UI state machine
- separate:
  - focus state
  - overlay state
  - pending async state
  - dialogue input state
- remove incidental behavior from event handlers

Deliverables:

- `src/ui/state.rs`
- `src/ui/events.rs`

Acceptance criteria:

- `Esc`, `Enter`, and modal transitions are driven by typed state transitions
- UI behavior can be tested without full terminal rendering

## Recommended New Module Layout

```text
src/
  domain/
    commands.rs
    events.rs
    ids.rs
    invariants.rs
    relationship.rs
    vocab.rs
    world.rs
  app/
    read_model.rs
    service.rs
  ai/
    context.rs
    policy.rs
    prompting.rs
    proposals.rs
    validation.rs
  ui/
    events.rs
    presenter.rs
    state.rs
    tui.rs
  persistence/
    mod.rs
```

## Test Strategy

### Domain Tests

- graph invariant tests
- procgen determinism tests
- command precondition tests
- event emission tests

### AI Contract Tests

- context fixture tests
- prompt construction tests
- proposal parsing tests
- proposal validation tests

### UI Tests

- presenter tests
- UI state-machine tests
- modal transition tests

### Persistence Tests

- save/load round-trip tests
- version migration tests

## Migration Order

Recommended order for implementation:

1. typed read models and presenter extraction
2. typed commands and events
3. graph invariant layer
4. AI proposal/validation split
5. semantic vocab types
6. versioned AI context contracts
7. structured relationship memory
8. persistence versioning
9. TUI state machine hardening

Rationale for the reordered next steps:

- the AI mutation boundary is the highest remaining architectural risk
- canonical vocabularies are still important, but they are safer to add once the AI path is no longer coupled directly to live state mutation

## Definition of Done

This refactor is complete when:

- no gameplay logic depends on formatted strings
- no AI output can mutate world state without proposal validation and policy checks
- world invariants are explicit and testable
- UI consumes typed read models only
- AI prompt input is a versioned typed contract
- persistence is versioned
- key gameplay flows are covered by deterministic tests

## Immediate Next Work

Start with Phase 5.

Reason:

- it removes the highest-risk remaining architectural shortcut
- it turns the LLM boundary from "trusted actions" into "validated proposals"
- it makes later work on typed vocabularies and versioned AI contracts safer
- it creates an auditable path for all AI-driven state change

The first concrete code change should be:

- add typed `AiProposal` types
- add a validator/policy step between LLM output and application state changes
- stop applying `WorldAction` directly inside `src/app/service.rs`
