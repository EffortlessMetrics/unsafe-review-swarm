# UNSAFE-REVIEW-SPEC-0012: LSP and editor projection

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md

## Problem

`unsafe-review` needs a precise, checkable behavior contract for lsp and editor projection.

## Behavior

LSP v1 is read-only: diagnostics, hover, status, copy agent packet, copy witness command, open related test.
The first supported surface is a saved JSON projection rendered with
`unsafe-review check --format lsp`. It is not an LSP server and it does not
edit source. The projection derives diagnostics, hovers, and command-style
action data from existing `ReviewCard`s. Diagnostics carry structured next
action, witness route details, verify commands, missing evidence, and the
static-review trust boundary. Diagnostics also include the concrete operation
expression from the `ReviewCard`, so editor consumers do not need to parse hover
text or reclassify findings. Hover text is a compact reviewer view: card
identity, required safety conditions, ReviewCard evidence summaries, missing
evidence, next action, verify commands when available, witness route, and the
static-review trust boundary.

## Non-goals

- no soundness claim
- no hidden blocking unless policy mode explicitly enables it
- no duplicate truth outside this spec and linked policy files

## Required evidence

- fixture-backed examples for positive and negative cases
- JSON output contract coverage
- human output smoke coverage
- policy documentation when behavior is configurable

## Acceptance examples

- A changed unsafe seam produces one review card with stable identity.
- The card includes missing evidence and a next action.
- If evidence is not knowable statically, the card names the limitation instead of overclaiming.
- `unsafe-review check --format lsp --out target/unsafe-review/lsp.json`
  writes read-only status data, diagnostics, hovers, and copy-command action
  data, including opening statically related tests when test-reach evidence is
  present.
- The projection includes no source edits and preserves the static-review trust
  boundary.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
cargo test -p unsafe-review-core lsp_projection
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
