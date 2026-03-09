# Procedural LLM City Sandbox on `rig`

## Summary
- Build a single-player, turn-based Rust game with a plain terminal CLI, using typed commands for exploration and freeform typed dialogue for conversations.
- Generate one seeded world at new-game time containing a connected network of 16-24 cities, each with districts, landmarks, factions, jobs, and resident NPCs.
- Use `rig-core` as the LLM layer for NPC conversation, with a provider-agnostic backend abstraction and first-class adapters for both local (`Ollama`) and hosted APIs.
- Treat v1 as a sandbox with emergent goals: rumors, favors, jobs, relationships, and discoveries replace a fixed campaign.
- Use the current CLI MVP as the base layer, then expand toward deeper simulation, stronger NPC continuity, and a larger set of player verbs without introducing a graphical UI yet.

## Key Implementation Changes
- Restructure the crate into a small binary bootstrap plus a testable library with four subsystems: `cli`, `world`, `simulation`, and `llm`.
- Drive the game through a command loop with explicit modes instead of an evented UI framework. Required gameplay modes: startup, overworld exploration, city inspection, travel selection, dialogue, journal review, and save/load.
- Implement a command grammar for v1 so the interface stays deterministic and testable. Core commands should include `help`, `look`, `where`, `travel`, `people`, `talk <npc>`, `ask <text>`, `journal`, `wait`, `save`, `load`, and `quit`.
- Dialogue should switch the CLI into a conversation context where the player can type normal sentences directly; use a small set of escape commands such as `/leave`, `/people`, and `/repeat` so freeform input does not collide with system commands.
- Represent the world as a graph, not a tile map. Each `City` stores biome/economy/culture tags, districts, landmarks, connected cities, and notable NPC ids. Travel consumes turns based on edge distance.
- Generate NPCs procedurally from city context. Each `Npc` should include identity, archetype, personality traits, goals, occupation, home/work anchors, relationship seeds, known rumors, and a compact memory ledger for the player.
- Advance time in discrete turns. Actions such as travel, inspect, and each dialogue exchange consume time and trigger world simulation updates: NPC schedule shifts, rumor spread, relationship decay/growth, and occasional new opportunities.
- Keep persistence simple: save a full materialized world snapshot plus a seed and transcript/journal history in local `serde` files. No database in v1.
- Define public core types up front so the implementation is stable: `GameState`, `World`, `City`, `District`, `Npc`, `Relationship`, `Rumor`, `DialogueSession`, `WorldAction`, `WorldEvent`, and `PlayerJournalEntry`.
- Add an `LlmBackend` trait that hides the chosen Rig provider and exposes `generate_dialogue`, `stream_dialogue`, and `summarize_memory`. Implement adapters for `Ollama` and one hosted provider first; keep the config model generic enough to add more Rig providers without touching game logic.
- Drive NPC talk through a conversation service, not directly from the CLI parser. Each request should assemble prompt context from NPC profile, current city, recent world events, relationship state, discovered facts, and recent transcript.
- Implement NPC “freeform agency” as open-ended dialogue text plus validated game-side mutations. The LLM may propose actions, but persistent state changes must pass through a constrained tool/extraction layer so the world remains coherent.
- Use Rig tools or structured extraction for the mutation layer. Supported v1 mutations should be limited to: revealing facts, adding rumors, changing relationship values, offering a job/favor, changing known locations, transferring a simple item, and scheduling a future meeting.
- Enable Rig multi-turn tool handling for dialogue flows that need more than one tool round-trip, and stream reply text line-by-line to stdout so the CLI does not feel blocked on full completion.
- Add a journal/rumor system as the player-facing progression loop. Every accepted rumor, promise, discovered landmark, and named contact should appear in a readable journal without requiring the player to remember transcript details.
- Keep combat, inventory depth, economics, and party systems out of v1. If items exist at all, keep them lightweight and conversation-driven.

## Future Features
- Add a stronger city simulation layer with local factions, public events, district reputations, shortages, and city-specific tensions that can change over time.
- Expand NPC modeling to include schedules, social ties, faction membership, recurring locations, rivalries, and longer-lived memory summaries so repeated conversations stay coherent across many turns.
- Add more player verbs beyond `look` and `travel`, such as `visit <district>`, `investigate <lead>`, `listen`, `rest`, `work`, `trade`, and `follow <npc>`.
- Introduce lightweight inventory and evidence systems so conversations can reference items, letters, maps, favors owed, and proof the player has uncovered.
- Add generated jobs and rumor chains that can branch across several cities, with deadlines, partial completion states, and multiple ways to resolve them through travel or dialogue.
- Support meetings, appointments, and time-of-day windows so specific NPCs are easier or harder to find depending on the turn and location.
- Add faction reputation and city-wide trust systems so the player’s behavior with one group affects prices, access, information quality, and risk in related cities.
- Expand the travel layer with road conditions, travel costs, random encounters, caravans, ferries, or dangerous routes that create more variation between cities.
- Add richer save metadata and multiple save slots, including save labels, timestamps, and a short world summary shown before loading.
- Add optional procedural “story seeds” at new-game time, such as missing couriers, succession disputes, cult activity, labor unrest, or smuggling crackdowns.

## Technical Improvements
- Split the current `simulation` module into smaller focused modules for command parsing, world updates, persistence, dialogue orchestration, and journal handling to reduce coupling.
- Replace ad hoc string matching in the CLI with a typed command parser and structured command enum so new verbs can be added without growing the main input handler into a monolith.
- Strengthen the LLM action-validation layer by separating extraction from application and logging rejected actions for debugging prompt quality and model drift.
- Add explicit `WorldEvent` and `QuestLead` domain types so rumors, favors, meetings, and scheduled consequences can be simulated consistently instead of only journaled as text.
- Persist NPC memory summaries, discovered city facts, and active leads in more structured forms so future systems can reason over them without reparsing journal strings.
- Add a deterministic simulation step runner that can process time advancement, rumor spread, travel consequences, and NPC relationship drift even when the player is not in dialogue.
- Improve provider configuration with explicit CLI/env validation, clearer startup errors, configurable model names, and tunable temperature/max-turn settings.
- Add transcript truncation and summarization policies so long conversations remain affordable with local or hosted models while preserving key facts for future prompts.
- Add prompt snapshots or debug logging behind a feature flag so NPC prompt context can be inspected when dialogue quality regresses.
- Introduce benchmark and cost-observability hooks for dialogue generation so different models can be compared on latency, token usage, and extraction reliability.

## Test Plan
- Unit-test procgen determinism from a fixed seed, including graph connectivity, city count bounds, and NPC generation invariants.
- Unit-test simulation rules for turn advancement, travel costs, rumor propagation, and relationship updates.
- Unit-test command parsing and mode transitions, including ambiguous input, invalid commands, and dialogue-mode escape commands.
- Unit-test save/load round-trips so a saved world reproduces the same player position, NPC states, journal, and transcript summaries.
- Integration-test the conversation pipeline with a fake `LlmBackend`: entering dialogue, submitting freeform text, receiving streamed chunks, applying validated world mutations, and updating the journal.
- Integration-test invalid or overly broad LLM mutation proposals to confirm they are rejected without corrupting world state.
- Add manual acceptance scenarios for: starting a new game, discovering valid commands through `help`, traveling across multiple cities, meeting NPCs with distinct personalities, unlocking rumors through conversation, saving, loading, and continuing the same conversation history.
- Add future regression tests for richer NPC context assembly, multi-city lead progression, scheduled meetings, faction reputation effects, and save compatibility across content updates.

## Assumptions
- V1 is terminal-only and uses a plain stdin/stdout command loop; there is no `bevy`, no `ratatui`, and no graphical renderer.
- “Many cities” means one pre-generated world with 16-24 cities, not infinite expansion.
- “Local or any API” means the code should default to a provider-agnostic config layer and ship with at least `Ollama` plus one hosted Rig provider adapter.
- “Full freeform agency” is interpreted as unconstrained player input and expressive NPC output, while durable game-state mutations still go through validated structured actions.
- The game is English-only, single-player, and offline-saveable in v1.
