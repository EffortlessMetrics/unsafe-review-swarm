# Contributing: source-of-truth migration

When proposing or implementing source-of-truth changes, keep durable rails in repo-owned namespaces and keep tool-runtime state separate.

## Owned scope

Primary durable scope for this repository:

- `.allow/` cargo-allow spec-system graph and active goal
- `docs/` source-of-truth artifacts
- `plans/` implementation plans
- `policy/` enforcement ledgers and references

## External state (awareness-only)

Do not store durable source-of-truth artifacts under:

- `.codex/`
- `.spec/`
- `.claude/`
- `.jules/`

These directories may exist for external tools, but this lane does not migrate, rewrite, or depend on them as owned state. `.rails/` is also retained as a read-only parity archive during the migration window.

## Practical rules

1. Keep proposal/spec/ADR/plan/proof/closeout responsibilities distinct.
2. Link artifacts with stable IDs where available.
3. Route product claims through support tiers and policy ledgers.
4. Add one PR-sized change at a time with explicit proof commands.

## Directory Intent

- `.allow/goals/`: current and archived goal metadata.
- `.allow/artifacts/`: registered proposal/spec/plan/support/closeout graph.
- `.allow/profiles/`: cargo-allow spec-system configuration.
- `.rails/`: legacy parity snapshot; do not route new work here.
- `docs/proposals/`: why a workstream exists, alternatives, and success criteria.
- `docs/specs/`: behavior and evidence requirements.
- `docs/adr/`: durable architecture decisions.
- `docs/templates/`: reusable proposal, spec, plan, closeout, swarm digest, and receipt skeletons.
- `docs/status/`: support posture, objective audits, dogfood lane status, and closeout-facing status notes.
- `policy/`: live baselines, suppressions, ledgers, and policy references.

Prefer focused work items under the active `.allow` goal rather than parallel
trackers. Keep durable graph links in `.allow/artifacts/doc-artifacts.toml` so
cargo-allow can validate them.

## Spec lifecycle and management

Specs are permanent, load-bearing parts of the codebase, not throwaway planning.
Managing them is managing the design, so treat it as a first-class activity, not
cleanup trivia.

### Status lifecycle

Every spec carries a `Status:` field (gated by `check-spec-status`):

- `draft` — proposed, not yet authoritative.
- `accepted` / `active` — the current contract; downstream work conforms to it.
- `superseded` — replaced by a newer spec; kept for provenance, no longer
  authoritative.
- `deprecated` — the behavior is being removed.

A stale spec — one that still reads `accepted` but no longer matches the code —
is a bug. Update it or mark it `superseded`; never leave it as false authority.

### Preserve detail, maintain the ledger

When a spec is replaced, mark it `superseded` and link the replacement; do not
delete it. The detailed specs, lanes, ADRs, and closeouts are the full
provenance — how the design got here — and are preserved. Current truth is
carried forward by the maintained ledgers (`docs/status/SUPPORT_TIERS.md` for
claim→proof and `.allow/artifacts/doc-artifacts.toml` plus `.allow/goals/` for
active state), which
link down into the detail. Re-organize by maintaining those ledgers over the
preserved detail, never by deleting lanes — that avoids both archaeology (no
index) and amnesia (deleted history).

### Single source

One fact lives in one canonical spec; other docs link to it rather than
restating it. A restated fact is a second truth surface that drifts. (This is
why no separate "operating ledger" doc was added: `CLAUDE.md`, `AGENTS.md`, and
`.allow/goals/active.toml` and the cargo-allow worklist already are the
cold-entry control panel; duplicating
them would drift.)

### Specs are directional, not binding

A spec is the current best-verified understanding, not an axiom — it can be
wrong. An agent that faithfully implements a false premise produces a wrong
result that passes its own re-blessed checks. Verify a spec's premise against
the code before building on it; when verification falsifies it, update the spec
rather than shipping to it. See `AGENT-ORCHESTRATION.md` §11–12.

### Enforcement

`check-spec-status`, `check-docs`, `check-support-tiers`, and
`check-doc-artifacts` keep the lifecycle honest (status present and valid,
front-door wording, every claim names its proof, the artifact graph links). A
correction to a spec goes through the same gates as any other change.
