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
- spec lifecycle dashboard,
- docs map,
- public product surfaces,
- published crate docs/readmes,
- release/publication receipt shape,
- no-overclaim trust-boundary wording,
- command snippets that reference `unsafe-review` or `xtask` commands.

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
- If a public README claims a product boundary, it must include no-proof/no-UB-free/no-Miri-clean wording.
- If a crate README references a local asset, `cargo package --list` must include it.
- If a doc-artifact ledger references a file, that file must exist.
- If a work item is active or ready, it must list proof commands.

## Follow-up

Start with machine-checking for the spec dashboard and docs surface ledgers, then
add generation commands only after the checker outputs are trusted.
