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
the canonical `OperationFamily` and `HazardKind` vocabulary. Each hazard,
obligation evidence key, and witness route kind must belong to the operation
family's registry row, and a card must not duplicate a hazard entry. Top-level
evidence summaries remain for compatibility and human scanning. Cards also
expose the ReviewCard's structured witness routes and next-action summary so
JSON consumers can route the same reviewer action as PR summaries, SARIF, LSP
hovers, and agent packets without reclassifying findings. Command-bearing
witness routes must name the matching witness tool; manual or unsupported
route kinds stay commandless by default. The next-action summary is a reviewer
instruction, not a verdict: it must be non-empty, concrete, operation-aware
when it names a safety obligation, and free of "all clear", safety-proof,
UB-free, Miri-clean, or site-execution claims.
Rendered ReviewCard JSON also includes a `confirmation_cue` object with the
same hypothesis, build-this-first, minimal-repro, confirmation-step, and trust
boundary projection used by comment-plan and agent packets. The cue is
plan-only: it must not imply that unsafe-review ran the command, observed
runtime behavior, proved site execution, proved UB, or proved repository
safety.

Card `class`, `priority`, `confidence`, and `proof_path` values must use the
canonical ReviewCard vocabulary. `proof_path` is a reviewer routing hint for the
kind of evidence that could make a card reviewable, not a proof claim and not a
verdict that a witness has run. Fixture goldens pin the expected signal for
supported classification states so a card cannot silently drift from a
high-confidence contract gap to a weaker or unknown PR-review signal.

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
- The card's operation family and hazards use canonical domain vocabulary,
  hazards, obligation evidence keys, and witness route kinds belong to the
  operation family registry row, command-bearing witness routes name their
  matching tool, and hazards are not duplicated.
- The card's class, priority, confidence, and proof path are known ReviewCard
  values and match the fixture-pinned classification signal.
- The card includes missing evidence and a next action.
- The card's next action names a concrete review step without implying safety,
  UB-free status, Miri-clean status, or site execution.
- The card's rendered JSON includes a confirmation cue that frames the finding
  as a static hypothesis and names the first build/run or witness-route cue
  without claiming it was executed.
- If evidence is not knowable statically, the card names the limitation instead of overclaiming.
- Fixture `expected.cards.json` files pin the rendered card JSON for supported smoke cases.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
