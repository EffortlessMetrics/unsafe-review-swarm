# UNSAFE-REVIEW-SPEC-0032: Comment-plan coverage-gap hardening

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
Support-tier impact: comment-plan projection surface
Policy impact:
- none

## Problem

`unsafe-review` already emits `comment-plan.json`, a bounded, plan-only inline
comment artifact (UNSAFE-REVIEW-SPEC-0022). It is the right shape — sparse,
deduped, changed-line scoped, advisory — but it predates the coverage model. Its
selection reasons speak in priority/confidence terms, not in **which coverage
slot is weak or missing**, so a posting wrapper cannot tell a reviewer *why this
seam is under-reviewed* from the plan alone. The plan is the projection a
poster trusts verbatim; today it leaks "actionable high-confidence card" rather
than "guard coverage is weak here."

`unsafe-review` is the coverage instrument (UNSAFE-REVIEW-SPEC-0028); the
comment plan is one projection of the per-card coverage block defined in
UNSAFE-REVIEW-SPEC-0029. This spec extends SPEC-0022 to anchor every comment to
that coverage block and to rail-cage any generated body. It hardens the plan
artifact; it does not change the posting model.

`comment-plan.json` is the durable plan in every deployment, and posting is
opt-in and off by default (UNSAFE-REVIEW-SPEC-0028 boundary). In `unsafe-review`'s
standalone PR-gate mode, an explicit opt-in posting path consumes this plan and
posts it (the trusted-poster split-token / idempotency model in
`docs/ci/TRUSTED_COMMENT_POSTER.md`). When `unsafe-review` runs inside
`ub-review`, the LLM layer posts and `unsafe-review` only emits the plan. Either
way the plan is the source of truth a poster trusts verbatim — the hardening
below makes that plan safe to post unmodified, whoever posts it.

## Behavior

This is artifact hardening of the existing plan-only `comment-plan.json`, not a
new surface and not analyzer expansion. All of SPEC-0022's contract (max 3,
plan-only/advisory, every card in `comments[]` or `not_selected[]`, renderable
`path`+`line`, closed-vocabulary reasons) still holds. The additions:

### Every selected comment names its coverage gap

A `selected` candidate must reference the coverage slot it is about — exactly one
of the SPEC-0029 slots that is `weak` or `missing`: `contract_coverage`,
`guard_coverage`, `test_reach_coverage`, or `witness_receipt_coverage`. The
`selection_reason` must reference that slot. The plan does not recompute coverage;
it reads the card's coverage block and projects it. A verifier checks the named
gap matches the referenced card's coverage block (the projection-parity rule of
SPEC-0029).

### Candidate identity and coverage fields

Each candidate carries, in addition to the SPEC-0022 fields:

```text
card_id OR manual_candidate_id   identity (one of; manual-candidate per SPEC-0027)
path, line                       renderable anchor
changed_line                     changed-line status (bool)
coverage_gap                     the weak/missing SPEC-0029 slot this is about
why_selected                     selection_reason referencing coverage_gap
confirmation_state               surfaced per candidate (per SPEC-0030 movement)
```

Each `not_selected[]` entry additionally carries a required `reason_code` from
the SPEC-0022 closed vocabulary; an entry with no `reason_code` is rejected.

### Changed-line scoped by default

Comments are changed-line scoped by default. A summary-only (non-changed-line)
candidate is permitted only with an explicit recorded reason; absent that reason
a non-changed-line entry stays in `not_selected[]` with `outside changed hunk`,
unchanged from SPEC-0022.

### Forbidden-claim check (the LLM-review rail-cage)

Any generated body fills the bounded, identity-anchored slots above and never
free-writes. A forbidden-claim check rejects bodies whose text contains
proof / UB-free / Miri-clean / site-execution wording (or equivalents). This is
the rail-cage that lets a wrapper trust a generated body verbatim: the generator
fills slots, the check guarantees no boundary-violating claim survives into the
plan.

### Confirmation state per candidate

The `confirmation_state` from UNSAFE-REVIEW-SPEC-0030 is surfaced per candidate
so a reviewer sees whether the gap is `new`, `worsened`, `inherited`, or
`resolved` movement without re-deriving it. It is read, not computed here.

## Non-goals

This spec does not:

- add posting code, a token model, or a VCS API client in *this* spec (posting
  is opt-in and off by default per SPEC-0028: standalone gate mode posts the
  plan via the trusted-poster path, the orchestrator posts when embedded; this
  spec only hardens the plan both consume),
- add a token model, cost model, or LLM provider integration,
- widen analyzer detection, add hazard families, or change ReviewCard /
  manual-candidate identity,
- compute coverage, `baseline_state`, or `outcome_movement` (read from SPEC-0029
  / SPEC-0030),
- introduce blocking, suppression insertion, source edits, witness execution, or
  any proof / UB-free / Miri-clean / site-execution / calibrated
  precision-recall / policy-readiness claim.

## Trust boundary

`comment-plan.json` stays a plan-only advisory projection of static unsafe
coverage evidence. A named `coverage_gap` describes weak or missing *review
evidence* for a seam, not a defect, and not UB. The plan names what a reviewer
should look at; it does not claim memory-safety proof, UB-free status,
Miri-clean status, or site execution, and it does not post or block. Cross-tool
contracts with the orchestrator and siblings are governed by
`docs/interop/sibling-tools.md`.

## Proof obligations

- `cargo test -p unsafe-review-core` — coverage-gap anchoring, candidate field
  presence, changed-line-default scoping, forbidden-claim rejection, and
  `confirmation_state` surfacing on `comment-plan.json`.
- `cargo run --locked -p xtask -- check-first-pr-artifacts <dir>` — rejects a
  selected candidate whose `selection_reason` does not reference a `weak`/
  `missing` coverage slot, a candidate missing identity / `coverage_gap`, a
  `not_selected[]` entry missing `reason_code`, a body containing forbidden
  proof / UB-free / Miri-clean / site-execution wording, and projection drift
  from the referenced card's SPEC-0029 coverage block.
- `cargo run --locked -p xtask -- check-pr`.

## Machine check

Registered in `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` and routed from
`docs/specs/UNSAFE-REVIEW-SPEC-START-HERE.md`; validated by
`cargo run --locked -p xtask -- check-spec-status` and
`cargo run --locked -p xtask -- check-docs`.
