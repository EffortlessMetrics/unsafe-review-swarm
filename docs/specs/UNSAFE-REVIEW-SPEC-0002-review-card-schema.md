# UNSAFE-REVIEW-SPEC-0002: Review card schema

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md

## Problem

`unsafe-review` needs a precise, checkable behavior contract for review card schema.

## Behavior

ReviewCard is the canonical unit for CLI, PR, SARIF, LSP, badges, and agent packets.
Machine-readable review-card and agent-packet JSON output is produced through
serde-backed DTOs so escaping, arrays, and required fields are parser-checked rather
than hand-rendered strings.
Cards include the concrete operation expression, operation family, and an
`obligation_evidence` array that reports contract, discharge, reach, and witness
state per safety obligation. Top-level evidence summaries remain for
compatibility and human scanning. Cards also expose the ReviewCard's structured
witness routes and next-action summary so JSON consumers can route the same
reviewer action as PR summaries, SARIF, LSP hovers, and agent packets without
reclassifying findings.

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
- Fixture `expected.cards.json` files pin the rendered card JSON for supported smoke cases.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
