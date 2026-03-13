
- Backwards compatibility is not a priority by default.
- Do not add compatibility shims, migration layers, or deprecated code paths
- Prefer simpler, cleaner, more type-safe architecture over preserving legacy behavior.
- Favor type safety over string matching.
- It is fine to refactor aggressively when it improves correctness or architecture.
- Update docs and tests to match the new behavior rather than preserving outdated interfaces.
- If a breaking change is intentional, keep the implementation clean instead of layering temporary compatibility hacks on top.
