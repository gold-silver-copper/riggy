# Architecture Roadmap

## Goal

Move the game from a prototype architecture to a production-grade, type-safe system for AI-driven gameplay.

The main constraints for this roadmap are:

- no string-matching control flow
- no UI copy embedded in domain logic
- no direct LLM-to-world mutation path
- graph state remains authoritative
- AI context and memory are explicit and testable

## Current Status

Completed phases:

- [x] Phase 1: typed read-model and presenter boundary
- [x] Phase 2: typed command/event application boundary
- [x] Phase 3: graph-authoritative identity and invariants
- [x] Phase 4: canonical semantic vocabularies
- [x] Phase 5: AI dialogue boundary without direct world mutation
- [x] Phase 6: typed AI context contracts
- [x] Phase 7: conversation memory

Not started:

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
- `src/ai/context.rs` defines the typed AI context contracts used by NPC dialogue
- `src/ai/prompting.rs` defines the prompt builders that consume those typed contracts
- `src/llm.rs` now consumes `NpcDialogueContext` instead of assembling ad hoc prompt-facing request structs
- `src/domain/memory.rs` defines `ConversationMemory`
- `GameState` stores per-NPC conversation memory
- `src/llm.rs` summarizes dialogue into conversation memory only
- AI context and UI read models now include conversation history summaries without any affinity/disposition mechanic

Note:

- Earlier roadmap phases discussed AI proposal validation and affinity mechanics. Those systems were removed in favor of a simpler production baseline: NPCs only retain conversation memory, and LLM output does not propose or apply world mutations.

## Current Problems

### 1. Persistence is not versioned

Save/load works, but persisted state is not schema-versioned yet. That makes future refactors riskier than they need to be.

### 2. The TUI runtime still relies on an ad hoc UI state machine

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
- conversation memory summarization

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
- [x] introduced typed application events for travel, dialogue lifecycle, vehicle entry/exit, inspection, and waiting
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

Remove duplicated identity and state assumptions.

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
- `src/llm.rs` now works with typed semantic fields through the typed AI context contract.
- procgen still produces freeform flavor text such as district descriptions, place descriptions, names, and landmarks, but canonical simulation semantics are now separate from that flavor text.

### Phase 5: Simplify the AI Boundary

Goal:

Ensure the LLM cannot directly mutate authoritative state.

Progress:

- [x] removed direct LLM-to-world mutation paths
- [x] constrained the AI layer to dialogue generation and conversation-memory summarization
- [x] kept authoritative state mutation inside the application service
- [x] added regression coverage for dialogue submission and memory persistence without AI-driven world actions

Status:

- Phase 5 is complete.
- The LLM boundary no longer attempts to mutate the world at all.

Work:

- remove any direct mutation path from the LLM layer
- keep LLM responsibilities limited to:
  - dialogue text generation
  - conversation summarization
- route all durable state mutation through typed application code

Deliverables:

- simplified `src/llm.rs`
- typed AI context integration through `src/ai/context.rs`

Acceptance criteria:

- no LLM output is applied directly to state
- AI responsibilities are narrow and testable

Completion notes:

- `src/llm.rs` now returns dialogue text only.
- conversation summarization is the only non-dialogue output from the AI layer.
- `src/app/service.rs` remains the only layer that mutates durable gameplay state.
- no proposal, policy, or validation submodules remain in the AI layer.

### Phase 6: Typed AI Context and Prompt Contracts

Goal:

Make AI context explicit, stable, and testable.

Progress:

- [x] added `src/ai/context.rs` with `NpcDialogueContext`, `DialogueTurnContext`, and `ConversationMemoryView`
- [x] added `src/ai/prompting.rs` with prompt builders that consume only typed context structs
- [x] updated `src/llm.rs` to accept `NpcDialogueContext` instead of an ad hoc request type
- [x] moved prompt-shape tests to context-fixture tests that do not require live world state
- [x] updated the application service to build typed AI context through `build_npc_dialogue_context`

Status:

- Phase 6 is complete.
- The LLM boundary now consumes an explicit typed context contract instead of a prompt-facing request struct assembled directly inside `src/llm.rs`.

Work:

- add typed context objects:
  - `NpcDialogueContext`
  - `DialogueTurnContext`
  - `ConversationMemoryView`
- add a prompt builder that consumes only those contracts
- separate authoritative facts from presentation text

Deliverables:

- `src/ai/context.rs`
- `src/ai/prompting.rs`

Acceptance criteria:

- AI requests are built from typed context structs
- prompt tests use context fixtures instead of live game state
- changing prompt shape does not require touching simulation logic

Completion notes:

- `src/ai/context.rs` now owns the dialogue context contract.
- the AI contract now owns its own transcript line and speaker types instead of embedding simulation transcript structs.
- `src/ai/prompting.rs` now owns dialogue prompt rendering for `NpcDialogueContext`.
- `src/llm.rs` now consumes `NpcDialogueContext` directly, and no longer owns the prompt-facing request contract.
- prompt rendering tests now use hand-built context fixtures instead of constructing a live `World`.
- the application service now builds AI context through `build_npc_dialogue_context`, keeping prompt-shape assembly out of the LLM adapter layer.
- `build_npc_dialogue_context` now derives city and NPC facts from authoritative world ids and rejects incoherent city/NPC/session combinations.
- dialogue clock values in `NpcDialogueContext` now come from authoritative game time instead of transcript-length heuristics.

### Phase 7: Add Conversation Memory

Goal:

Store durable conversation state in a form the game can reason about.

Work:

- replace ephemeral transcript-only memory with a typed conversation summary object
- merge new summaries across conversations without overwriting prior context
- keep the shape minimal and focused on what was discussed

Deliverables:

- `src/domain/memory.rs`
- updated summarization path

Acceptance criteria:

- NPC memory contains durable conversation context
- AI context can include conversation summaries

Status:

- Phase 7 is complete.
- Conversation memory is now a typed domain object instead of a single summary string.

Completion notes:

- `src/domain/memory.rs` now owns `ConversationMemory` with normalization helpers.
- conversation memory updates are merged durably across conversations instead of overwriting prior context.
- `NpcMemoryState` stores `memory: ConversationMemory`.
- `src/llm.rs` now summarizes conversations into `ConversationMemory` for both mock and Rig backends.
- `src/ai/context.rs` now includes conversation memory in `NpcDialogueContext`.
- `src/ai/prompting.rs` now renders conversation memory into dialogue prompts.
- `src/app/read_model.rs` and `src/presenter.rs` now project and render conversation memory.
- dialogue exit now preserves the active session if summarization fails, instead of discarding the conversation before the await succeeds.
- tests now cover memory normalization, AI context mapping, and service-level persistence of memory after dialogue.

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
    memory.rs
    vocab.rs
    world.rs
  app/
    read_model.rs
    service.rs
  ai/
    context.rs
    prompting.rs
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
- conversation summarization tests

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
4. semantic vocab types
5. simplified AI boundary
6. typed AI context contracts
7. conversation memory
8. persistence versioning
9. TUI state machine hardening

## Definition of Done

This refactor is complete when:

- no gameplay logic depends on formatted strings
- no AI output can mutate world state directly
- world invariants are explicit and testable
- UI consumes typed read models only
- AI prompt input is a typed contract
- persistence is versioned
- key gameplay flows are covered by deterministic tests

## Immediate Next Work

Start with Phase 8.

Reason:

- the AI and memory contracts are now typed enough that save schema churn becomes the main structural risk
- persistence is still the weakest architectural boundary left in the core runtime
- versioned saves are the cleanest next step before more schema-heavy work lands

The first concrete code change should be:

- add a versioned persisted save wrapper
- separate persisted gameplay state from runtime-only UI/application state
- define explicit migration entry points for future schema changes
