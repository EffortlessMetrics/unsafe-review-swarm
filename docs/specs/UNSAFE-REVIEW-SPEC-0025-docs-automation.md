# UNSAFE-REVIEW-SPEC-0025: docs automation

- Status: proposed
- Owner: repo-infra
- Created: 2026-05-21
- Linked proposal: [UNSAFE-REVIEW-PROP-0002](../proposals/UNSAFE-REVIEW-PROP-0002-source-of-truth-stack.md)
- Linked specs:
  - [UNSAFE-REVIEW-SPEC-0020](UNSAFE-REVIEW-SPEC-0020-source-of-truth-stack.md)
- Support-tier impact: `docs/status/SUPPORT_TIERS.md`
- Policy impact:
  - `policy/doc-artifacts.toml`
  - `policy/docs-automation.toml`
  - `policy/public-surfaces.toml`

## Problem

`unsafe-review` has a growing set of public docs, specs, status pages, release
receipts, support tiers, and published crate surfaces. Several of those surfaces
are product-critical. Manual upkeep alone is not enough: stale docs can create
false product claims, broken first-use paths, or ambiguous next-work guidance.

## Behavior

The repo MUST provide docs automation that checks or generates:

- source-of-truth artifact graph,
- active goal and work-item proof commands,
- lane trackers and implementation plans,
- spec lifecycle dashboard,
- agent operating contract,
- docs map,
- public product surfaces,
- published crate docs/readmes,
- release/publication receipt shape,
- no-overclaim trust-boundary wording,
- command snippets that reference `unsafe-review` or `xtask` commands.

Docs automation owns repo source-of-truth paths only. Roots listed as
`external_awareness_only` in `policy/docs-automation.toml` are context for
agents and tools, not durable unsafe-review state; they must not appear as owned
roots, checked/generated outputs, or source inputs for checked repo artifacts.

## Non-goals

- no migration into `.codex`,
- no mutation of `.spec`,
- no agent scratch-state ownership,
- no automatic claim promotion,
- no generated support-tier claim without proof,
- no runtime analyzer behavior changes.

## Required evidence

```bash
cargo run --locked -p xtask -- check-docs-automation
cargo run --locked -p xtask -- check-doc-artifacts
cargo run --locked -p xtask -- check-goals
cargo run --locked -p xtask -- check-support-tiers
cargo run --locked -p xtask -- check-pr
git diff --check
```

## Acceptance examples

- If a spec appears in `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md`, it must exist.
- If a spec row lists proof commands, those commands must use known `xtask`/CLI surfaces.
- `AGENTS.md` must preserve source/swarm routing, source-divergence preflight,
  source-of-truth stack, SPEC-0024 CI routing, and no-`.codex` durable-state
  wording.
- `AGENTS.md` must preserve the active improvement runway, expected generated
  PR batches, spec rails as forward drive, missing-rail alignment, and
  configuration-obstacle handling for single-contributor review gates.
- `policy/docs-automation.toml` must keep external agent/tool roots awareness
  only, not owned or checked source-of-truth paths.
- Active lane plans must preserve guardrail wording that matches the checked
  policy/verifier behavior they describe.
- If a public README claims a product boundary, it must include no-proof/no-UB-free/no-Miri-clean wording.
- If a crate README references a local asset, `cargo package --list` must include it.
- If a doc-artifact ledger references a file, that file must exist.
- If a work item is active or ready, it must list proof commands.

## Follow-up

Start with machine-checking for the spec dashboard and docs surface ledgers, then
add generation commands only after the checker outputs are trusted.
