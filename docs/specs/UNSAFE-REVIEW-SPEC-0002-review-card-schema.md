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
Cards include stable site metadata: relative Rust file path, positive line and
column, known site kind, owner, visibility, public API surface flag, and a
single-line snippet. The top-level `operation` string must match
`site.snippet`, keeping PR, editor, agent, SARIF, and receipt projections on the
same operation expression.
Cards include the concrete operation expression, operation family, hazards, and
an `obligation_evidence` array that reports contract, discharge, reach, and
witness state per safety obligation. `operation_family` and `hazards` must use
the canonical `OperationFamily` and `HazardKind` vocabulary, and a card must not
duplicate a hazard entry. Top-level evidence summaries remain for compatibility
and human scanning. Cards also expose the ReviewCard's structured witness routes
and next-action summary so JSON consumers can route the same reviewer action as
PR summaries, SARIF, LSP hovers, and agent packets without reclassifying
findings. The next-action summary is a reviewer instruction, not a verdict: it
must be non-empty, concrete, operation-aware when it names a safety obligation,
and free of "all clear", safety-proof, UB-free, Miri-clean, or site-execution
claims.

Card `class`, `priority`, and `confidence` values must use the canonical
ReviewCard vocabulary. Fixture goldens pin the expected signal for supported
classification states so a card cannot silently drift from a high-confidence
contract gap to a weaker or unknown PR-review signal.

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
- The card's site metadata uses known kind/visibility values, positive source
  coordinates, a relative Rust source path, and an operation string that matches
  the site snippet.
- The card's operation family and hazards use canonical domain vocabulary, and
  hazards are not duplicated.
- The card's class, priority, and confidence are known ReviewCard values and
  match the fixture-pinned classification signal.
- The card includes missing evidence and a next action.
- The card's next action names a concrete review step without implying safety,
  UB-free status, Miri-clean status, or site execution.
- If evidence is not knowable statically, the card names the limitation instead of overclaiming.
- Fixture `expected.cards.json` files pin the rendered card JSON for supported smoke cases.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
