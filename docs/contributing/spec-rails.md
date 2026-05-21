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

## Directory Intent

- `.unsafe-review-spec/goals/`: active and archived goal metadata.
- `.unsafe-review-spec/lanes/`: focused lane trackers and implementation sequencing.
- `docs/proposals/`: why a workstream exists, alternatives, and success criteria.
- `docs/specs/`: behavior and evidence requirements.
- `docs/adr/`: durable architecture decisions.
- `docs/templates/`: reusable proposal, spec, plan, closeout, and receipt skeletons.
- `docs/status/`: support posture, objective audits, dogfood lane status, and closeout-facing status notes.
- `policy/`: live baselines, suppressions, ledgers, and policy references.

Prefer focused lane trackers under `.unsafe-review-spec/lanes/` rather than one
global active queue. Keep durable rails indexed through `.unsafe-review-spec/index.toml`
when they need stable machine discovery.
