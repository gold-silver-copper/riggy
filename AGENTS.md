# AGENTS.md

This repository is in early alpha.

## Compatibility

- Backwards compatibility is not a priority by default.
- It is acceptable to break old saves, internal APIs, module layouts, and in-progress architecture if that materially improves the design.
- Do not add compatibility shims, migration layers, or deprecated code paths unless the task explicitly asks for them.
- Prefer simpler, cleaner, more type-safe architecture over preserving legacy behavior.

## Engineering Priorities

- Favor type safety over string matching.
- Prefer explicit domain types and validated state transitions.
- Remove dead code instead of preserving unused abstractions.
- Keep the graph/data model authoritative.
- Treat the current codebase as a moving prototype being shaped into a production-grade architecture.

## When Making Changes

- It is fine to refactor aggressively when it improves correctness or architecture.
- Update docs and tests to match the new behavior rather than preserving outdated interfaces.
- If a breaking change is intentional, keep the implementation clean instead of layering temporary compatibility hacks on top.
