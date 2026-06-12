# UNSAFE-REVIEW-SPEC-0029: Unsafe evidence coverage model

Status: proposed
Owner: product / core
Created: 2026-06-06
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- plans/0.2.0/implementation-plan.md
Linked issues:
- none
Linked PRs:
- TBD
Support-tier impact: ReviewCard projection surface
Policy impact:
- none

## Problem

`unsafe-review` projects ReviewCards into many surfaces (JSON, SARIF,
comment-plan, LSP, witness-plan, repair-queue, outcome, badges, agent packet),
but each surface re-derives "what is covered and what is missing" from scattered
fields (class, obligation evidence, witness state, next action). There is no
single machine-readable model of unsafe **coverage** that every surface reads
the same way.

The product is a coverage instrument (UNSAFE-REVIEW-SPEC-0028): its unit is "is
this unsafe seam reviewable?", not "is this UB?". To make badge, gate movement,
comment-plan, LLM packet, and the ub-review manifest all be projections of the
same measured coverage — rather than separate report formats that can drift —
the coverage state must be defined once on the card.

## Behavior

### Coverage slots

Each ReviewCard exposes a machine-readable coverage block. Each slot has a state
and a short summary; states are a closed vocabulary so consumers can compare and
rank without parsing prose.

```text
contract_coverage        present | weak | missing
guard_coverage           present | weak | missing
test_reach_coverage      present | weak | missing
witness_receipt_coverage present | missing            (present only via an imported receipt)
manual_context           present | absent             (manual-candidate overlay attached)
baseline_state           new | worsened | inherited | resolved | unknown   (per SPEC-0030)
outcome_movement         improved | regressed | unchanged | unknown        (per saved snapshot)
comment_plan_status      selected | not_selected | not_eligible
agent_lsp_readiness      ready | requires_witness_receipt | needs_human | unsupported
```

These reuse existing analyzer evidence (UNSAFE-REVIEW-SPEC-0006 contract and
discharge, UNSAFE-REVIEW-SPEC-0007 test reach, UNSAFE-REVIEW-SPEC-0009 receipts,
UNSAFE-REVIEW-SPEC-0013 agent readiness) — this spec **names and unifies** them
as one block, it does not widen analyzer detection. `weak` makes explicit a state
the analyzer already computes implicitly (evidence is present but does not
discharge the obligation).

### One projection, many surfaces

The coverage block is the canonical source for:

- the badge counts (open actionable = non-`present` actionable slots; the `+`
  count = `missing` plus `weak`),
- baseline movement (`baseline_state`, `outcome_movement`) per SPEC-0030,
- comment-plan selection reasons (a selected comment must name the coverage slot
  it is about) per SPEC-0032,
- the LLM context packet's missing-evidence and allowed-repair derivation per
  SPEC-0033,
- the `unsafe-review-gate.json` manifest summary per SPEC-0034.

No surface recomputes coverage from raw fields once this block exists; they read
it. A verifier checks that the projected coverage in each surface matches the
card's coverage block.

### Card statement shape

A card's coverage block lets a surface render the canonical statement:

```yaml
operation_family: stable_byte_source_getter_reentry
obligation: stable-byte stability
contract_coverage: present
guard_coverage: weak
test_reach_coverage: missing
witness_receipt_coverage: missing
baseline_state: new
worth_comment: true
```

Not "UB confirmed." The same block drives every consumer.

## Non-goals

This spec does not:

- widen analyzer detection or add hazard families,
- change ReviewCard identity (`UR-...-cN`) or manual-candidate identity,
- introduce execution, posting, blocking, or any proof / UB-free / Miri-clean /
  site-execution / calibrated precision/recall / policy-readiness claim,
- compute `baseline_state` or `outcome_movement` itself (those are supplied by
  SPEC-0030 baseline movement and saved-snapshot outcome comparison; this spec
  only carries the slots).

## Trust boundary

Coverage describes review evidence, not safety. `present` contract coverage means
the obligation's contract evidence was found, not that the code is correct.
`witness_receipt_coverage: present` reflects an imported receipt's claim, not
execution by `unsafe-review`. The coverage model makes reviewability legible; it
makes no safety, UB-free, Miri-clean, or site-execution claim.

## Proof obligations

- `cargo test -p unsafe-review-core` — coverage block derivation from existing
  evidence; closed-vocabulary state validation; `weak` vs `present` vs `missing`
  boundaries.
- projection-parity checks in `check-pr` / `check-first-pr-artifacts` — each
  surface's projected coverage matches the card's coverage block.
- `cargo run --locked -p xtask -- check-pr`.

## Machine check

Registered in `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` and routed from
`docs/specs/UNSAFE-REVIEW-SPEC-START-HERE.md`; validated by
`cargo run --locked -p xtask -- check-spec-status` and
`cargo run --locked -p xtask -- check-docs`.
