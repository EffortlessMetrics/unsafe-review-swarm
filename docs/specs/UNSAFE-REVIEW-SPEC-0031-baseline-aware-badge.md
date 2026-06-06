# UNSAFE-REVIEW-SPEC-0031: Baseline-aware repo badge

Status: proposed
Owner: product / cli
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
Support-tier impact: badge projection surface
Policy impact:
- none

## Problem

`unsafe-review badges --out badges/` already emits Shields.io endpoint JSON
(`schemaVersion` / `label` / numeric `message` / `color`) for two badges:
`unsafe-review` (open actionable gaps) and `unsafe-review+` (missing plus weak
evidence count), and tests already reject overclaim terms in the payload. What
is missing is the **consumer side**: there is no README snippet convention, no
main-branch refresh story, and no hosting guidance, so the counts a reader sees
go stale the moment the repo moves. And the raw counts are not baseline-aware —
on a mature repo the badge shows the full pre-existing debt, which is exactly
the muting failure mode UNSAFE-REVIEW-SPEC-0030 fixes for every other surface.

This is the repo-posture surface in the UNSAFE-REVIEW-SPEC-0028 surface table
(use case 1): the consumer is a README reader, the lifecycle moment is repo
posture (push-to-main / scheduled), and "easy" means **one README line that
stays fresh**. The badge is a projection of the per-card coverage block
(UNSAFE-REVIEW-SPEC-0029), not a new measurement.

## Behavior

### Counts are baseline-aware

The two badge counts read the coverage block (UNSAFE-REVIEW-SPEC-0029) measured
against the repo baseline floor (UNSAFE-REVIEW-SPEC-0030), not raw totals:

```text
unsafe-review   message = open-actionable count   (non-present actionable coverage slots)
unsafe-review+  message = missing + weak count     (missing plus weak evidence slots)
```

Both are measured against the baseline floor: a repo with a recorded baseline
shows movement-relevant open-actionable and missing+weak counts, not the full
inherited debt. With no baseline the badge falls back to raw open counts (the
honest "this repo has not set a floor yet" reading). The badge consumes
SPEC-0030 movement output; it does not recompute it.

### Payloads stay numeric-only and Shields-safe

The `message` field is the numeric count only. No safety word ever appears in
the payload — no "safe", "UB-free", "proof", "Miri-clean", "verified". `color`
is a coverage-pressure hint (more open-actionable -> warmer), not a verdict.
Meaning lives behind a linked status page / artifact (the movement report,
SPEC-0030), not in the badge text. The existing overclaim-term rejection test
stays authoritative and is the rail for this field discipline.

### Consumer adoption path

A documented, copy-pasteable README convention so the line stays fresh:

```text
![unsafe-review](https://img.shields.io/endpoint?url=<RAW_BASE>/badges/unsafe-review.json)
![unsafe-review+](https://img.shields.io/endpoint?url=<RAW_BASE>/badges/unsafe-review-plus.json)
```

`<RAW_BASE>` points at where the endpoint JSON is published off the main branch
(raw file host or pages artifact). The contract: the badge JSON is **regenerated
and published off `main` on push and on schedule**, never hand-run, so the count
a reader sees tracks the main-branch baseline floor. This spec describes that
refresh contract; the GitHub Action step that performs the regenerate-and-publish
belongs to UNSAFE-REVIEW-SPEC-0034 / the action lane, not here.

### The badge is a projection, not a gate

The badge reports posture for a reader. It runs no CI, posts nothing, blocks
nothing, and makes no pass/fail decision. A nonzero count is information, not a
failure; the gate decision belongs to the orchestrator (UNSAFE-REVIEW-SPEC-0028
boundary). Cross-tool badge/manifest contracts are shared per
[`docs/interop/sibling-tools.md`](../interop/sibling-tools.md).

## Non-goals

This spec does not:

- post the badge, open a PR, or write anywhere outside `--out`,
- block, gate, or change any exit code (the badge does not run CI),
- emit any safety / UB-free / Miri-clean / proof / site-execution /
  calibrated precision-recall / policy-readiness claim in the badge,
- define the publish Action step (UNSAFE-REVIEW-SPEC-0034 / action lane),
- recompute coverage or baseline movement (read from SPEC-0029 / SPEC-0030),
- add a new badge, hazard family, or detector.

## Trust boundary

The badge reports a **numeric coverage-gap count** measured against a recorded
baseline floor, nothing more. It is not a statement that the repo is memory-safe,
UB-free, Miri-clean, or that any unsafe site executed safely. A count of zero
means no open actionable unsafe-review gaps above the baseline, not "this crate
is safe." The badge text carries no safety vocabulary by construction.

## Proof obligations

- `cargo test -p unsafe-review-core` — baseline-aware count derivation from the
  coverage block; numeric-only `message`; overclaim-term rejection across both
  payloads; no-baseline raw-count fallback.
- `cargo test -p unsafe-review-cli` — `badges --out` writes both endpoint files
  with Shields-valid `schemaVersion` / `label` / numeric `message` / `color`.
- `cargo run --locked -p xtask -- check-docs` — README snippet convention is
  present and renders.
- `cargo run --locked -p xtask -- check-pr`.

## Machine check

Registered in `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` and routed from
`docs/specs/UNSAFE-REVIEW-SPEC-START-HERE.md`; validated by
`cargo run --locked -p xtask -- check-spec-status` and
`cargo run --locked -p xtask -- check-docs`.
