# Riggy

`riggy` is an early-alpha AI-driven text game built in Rust.

The current game loop is:

- one authoritative directed `petgraph` world in [`src/world.rs`](src/world.rs)
- one manually controlled actor that the UI presents as `You`
- colocated AI actors that choose actions through `rig` tool calls
- typed commands, events, read models, and presenter formatting between the world and the UI

This README is written as an operator/developer reference for working on `riggy` autonomously.

## Current Shape

- Runtime UI: ratatui in [`src/tui.rs`](src/tui.rs)
- Headless test client: [`src/bin/riggy_headless.rs`](src/bin/riggy_headless.rs)
- Authoritative application service: [`src/app/service.rs`](src/app/service.rs)
- Authoritative world graph: [`src/world.rs`](src/world.rs)
- AI turn context and prompting: [`src/ai/context.rs`](src/ai/context.rs) and [`src/ai/prompting.rs`](src/ai/prompting.rs)
- `rig` backend adapter and agent tools: [`src/llm.rs`](src/llm.rs)
- Read-model projection: [`src/app/read_model.rs`](src/app/read_model.rs)
- World prose and event rendering: [`src/presenter.rs`](src/presenter.rs)
- Logging bootstrap: [`src/logging.rs`](src/logging.rs)

## Binaries

### TUI

Run the main game:

```bash
cargo run -p riggy
```

### Headless

Run the scriptable headless client:

```bash
cargo run -p riggy --bin riggy_headless
```

Force the mock backend:

```bash
cargo run -p riggy --bin riggy_headless -- --mock
```

Run one-shot commands:

```bash
cargo run -p riggy --bin riggy_headless -- --mock \
  --command look \
  --command "say 0 hello"
```

Run a script file:

```bash
cargo run -p riggy --bin riggy_headless -- --mock --script scripts/smoke_test.riggy
```

## Backend Selection

Backend selection is currently environment-driven in [`src/llm.rs`](src/llm.rs):

- If `OLLAMA_MODEL` is set, `riggy` uses Ollama.
- Otherwise, if `OPENAI_API_KEY` or `OPENAI_BASE_URL` is set, `riggy` uses the OpenAI-compatible backend.
- Otherwise, `riggy` falls back to the local mock backend.

Useful environment variables:

- `OLLAMA_MODEL`
- `OLLAMA_API_BASE_URL`
- `OPENAI_API_KEY`
- `OPENAI_BASE_URL`
- `OPENAI_MODEL`
- `RUST_LOG`

Example Ollama run:

```bash
export OLLAMA_MODEL=qwen3:4b
cargo run -p riggy
```

Important:

- The debug panel shows the exact backend model, not just the provider.
- If the panel says `rig/ollama (llama3.2)`, you are still running `llama3.2`.
- `rig` can support Ollama tool calls, but individual local models may still ignore tools or use them badly.

## Logging

`riggy` now writes a root log file to:

```text
logs/riggy.log
```

The logger is initialized by both binaries in [`src/main.rs`](src/main.rs) and [`src/bin/riggy_headless.rs`](src/bin/riggy_headless.rs), using [`src/logging.rs`](src/logging.rs).

What the log captures:

- startup and backend selection
- action planning and execution
- autonomous NPC turn selection
- `rig` agent selection flow
- tool call arguments/results/errors
- TUI action submission and pending completion
- panics with backtraces

Default filter:

```text
riggy=trace,rig=debug,info
```

Override with `RUST_LOG`, for example:

```bash
RUST_LOG=riggy=trace,rig=trace cargo run -p riggy
```

`logs/` is ignored in git.

## TUI Debugging

The TUI has an agent debug panel.

- Press `F2` to toggle it.
- It shows the latest local NPC decision traces.
- It includes backend label, selected action, errors, available actions, recent speech, model output, and tool calls.

When debugging NPCs, the fastest loop is:

1. Reproduce in the TUI or headless client.
2. Check `logs/riggy.log`.
3. Check the agent debug panel or headless `debug` output.
4. Confirm whether the model made a real tool call or only emitted plain text.

## Headless Commands

The headless client currently supports:

- `look`
- `actions`
- `people`
- `routes`
- `entities`
- `context`
- `debug`
- `focus me`
- `focus <actor-id>`
- `travel <route-index-or-place-id>`
- `say <person-index-or-actor-id> <text>`
- `inspect <entity-index-or-entity-id>`
- `wait <30s|2m|1h>`
- `agent <actor-id>`
- `save <path>`
- `load <path>`
- `source <path>`
- `quit`

Notes:

- `people`, `routes`, and `entities` show both list indices and stable ids.
- `say 0 hello` talks to the first visible actor in the current place.
- `agent <actor-id>` forces one autonomous decision pass for that actor.
- `source` is top-level only; nested `source` commands inside script files are intentionally rejected.

Example script:

```text
# scripts/smoke_test.riggy
look
people
say 0 hello
debug
wait 30s
look
```

## Recommended Autonomous Development Workflow

If you are developing `riggy` without manually driving the TUI every time, use this loop:

1. Start with the headless client and the mock backend.
2. Reproduce the intended action flow with one-shot commands or a script file.
3. Inspect `logs/riggy.log`.
4. If the issue is AI-related, run `debug` in headless mode or use the TUI `F2` panel.
5. Only switch to Ollama/OpenAI-compatible once the game-side behavior is correct with the mock backend.
6. Use provider-backed runs to debug tool-calling and prompt behavior, not world-state logic.

Recommended command sequence:

```bash
cargo test -p riggy
cargo run -p riggy --bin riggy_headless -- --mock -c look -c "say 0 hello" -c debug
tail -n 200 logs/riggy.log
```

## Testing

Run the full crate tests:

```bash
cargo test -p riggy
```

Run a compile-only pass:

```bash
cargo check -p riggy
```

There are unit tests around:

- world invariants
- command planning
- speech/action flow
- autonomous NPC turn taking
- headless parsing and headless mock conversation flow

## Current Simulation Model

The game is no longer player-special-cased at the world level.

- The player is just the single `ControllerMode::Manual` actor.
- NPCs are `ControllerMode::AiAgent`.
- Both use the same typed action system.
- Actions are recorded as process nodes with duration in the graph.
- NPC decision making goes through `rig` tools, not direct LLM-to-world mutation.

Current actor actions include:

- move
- speak
- inspect
- wait
- do_nothing

This means:

- speech is a real action
- speech advances time
- NPC replies are separate actions, not magical side effects

## Known Operational Caveats

- Tool-calling quality depends heavily on the model. Smaller local models may still emit fake tool syntax in plain text.
- If an NPC appears idle, inspect both the debug panel and `logs/riggy.log` before assuming world logic is broken.
- The workspace still contains ontology-related crates, but the `riggy` game crate’s runtime logic lives in `src/`.
- Save/load exists, but persistence versioning is still an open roadmap item.

## Project Layout

High-signal paths:

- [`src/world.rs`](src/world.rs): authoritative game graph, generation, validation, processes
- [`src/domain/commands.rs`](src/domain/commands.rs): typed action API
- [`src/domain/events.rs`](src/domain/events.rs): typed result/event API
- [`src/app/service.rs`](src/app/service.rs): orchestration, action execution, autonomous turns
- [`src/llm.rs`](src/llm.rs): backend selection, agent tools, decision flow
- [`src/tui.rs`](src/tui.rs): ratatui runtime
- [`src/headless.rs`](src/headless.rs): headless automation client
- [`docs/architecture-roadmap.md`](docs/architecture-roadmap.md): architecture intent and unfinished items

## Short Version

If you only remember five things, remember these:

1. Use the headless client first.
2. Read `logs/riggy.log`.
3. Use the mock backend for game logic, real backends for tool-calling behavior.
4. Check the exact model name in the debug output before trusting any AI result.
5. Treat the graph in `src/world.rs` as authoritative.
