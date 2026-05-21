# Contributing: source-of-truth rails

When proposing or implementing source-of-truth changes, keep durable rails in repo-owned namespaces and keep tool-runtime state separate.

## Owned scope

Primary durable scope for this repository:

- `.unsafe-review-spec/`
- `docs/` source-of-truth artifacts
- `plans/` implementation plans
- `policy/` enforcement ledgers and references

## External state (awareness-only)

Do not store durable source-of-truth artifacts under:

- `.codex/`
- `.spec/`
- `.claude/`
- `.jules/`

These directories may exist for external tools, but this lane does not migrate, rewrite, or depend on them as owned state.

## Practical rules

1. Keep proposal/spec/ADR/plan/proof/closeout responsibilities distinct.
2. Link artifacts with stable IDs where available.
3. Route product claims through support tiers and policy ledgers.
4. Add one PR-sized change at a time with explicit proof commands.
