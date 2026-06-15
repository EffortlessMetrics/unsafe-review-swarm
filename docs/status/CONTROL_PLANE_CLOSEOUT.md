# Detector-discipline control-plane closeout (2026-06-15)

This records what the detector-discipline control-plane lane delivered: the
discipline ledgers and enforcing gates now in place, the findings it closed, the
exceptions that remain tracked, and what was deferred. It is a posture record,
not a claim of proof. The gates in this lane validate **process discipline and
ledger shape** — never memory-safety, UB-free, Miri-clean, site-execution, or
calibrated precision/recall properties. Every product output surface remains
advisory: no default merge-blocking on a user's PR, no comment posting, no source
edits. See [SUPPORT_TIERS](SUPPORT_TIERS.md) for the claim-to-proof ledger.

## Why this lane existed

The substring-anchoring bug class — a detector firing on a token without checking
unsafe scope, call-vs-definition, receiver/origin, string/comment masking, or
word boundary — is asymptotic under the syntax-first, build-free analyzer: each
anchoring fix tends to reveal the next gap. The right response is an invariant,
not a 51st point fix: a control plane that forces every detector to declare its
discipline obligations and negative controls, and every settled stance to keep
its proof — checked by a gate so a later change cannot silently weaken them.

## What landed

**Specs / ADR**

- ADR-0009 (syntax-first detection) → `active`.
- SPEC-0041 (syntax-first / semantic-light dispatch architecture), citing the
  SPEC-0005 appendix as the canonical D1–D5 discipline set.
- SPEC-0040 (detector-contracts ledger) → `accepted`; documents the enforcing
  gate and the documented-gap exception path.

**Discipline ledgers (`policy/`)**

- `detector-contracts.toml` — 11 high-risk family contracts (`get_unchecked`,
  `ptr_copy`, `copy_nonoverlapping`, `transmute`, `zeroed`, `vec_set_len`,
  `unsafe_fn_call`, `ffi`, and four `stable_byte_source_*` families), each
  declaring its D1–D5 obligations, positive/negative control fixtures, and
  projecting surfaces.
- `stance-decisions.toml` — 7 settled stances, each with rationale / owner /
  linked tests / linked spec / review-after.
- `spec-coverage.toml` — single-truth projection map: 8 fields → canonical
  pipeline source → projecting surfaces.

**Enforcing gates (xtask, in the `check-pr` bundle)**

- `check-detector-contracts`, `check-stance-decisions`, `check-spec-coverage`.
  Born informational (PR-5), populated (PR-6), and — once the two proof-gaps were
  closed — flipped to enforcing (PR-C). Each fails `check-pr` on a structural
  violation or an undocumented gap; a documented exception (owner + review_after
  + explanation) passes as a tracked warning. A `single_truth=false` projection
  is always blocking — a single-truth violation is a defect to fix, not a gap to
  ledger.

## Findings closed

- **zeroed / ffi negative-control gaps** (PR-A, #1731): the `zeroed` contract now
  references its existing definition-header and safe-wrapper negative controls; a
  new `ffi_safe_wrapper_only_no_cards` diff-scoped negative control proves the FFI
  detector stays silent when the FFI seam is off the changed lines. The
  `unsafe_fn_call`-owned `*_not_route` fixtures were not cross-claimed.
- **baseline_state single-truth violation** (PR-B, #1730): `policy_report` now
  projects the canonical `CoverageBlock` `baseline_state` — the same value the
  `json` and `agent` surfaces project — instead of re-deriving its own
  vocabulary. The policy classification stays in the separate `policy_status` /
  `policy_reason` fields, so no information was lost.

After PR-C the gates report: `check-detector-contracts` 11 contracts / 0 tracked;
`check-spec-coverage` 8 fields / 0 blocking; `check-stance-decisions` 7 stances /
2 tracked exceptions.

## Exceptions that remain (tracked, by design)

Two stance proof-gaps are recorded honestly rather than papered over — the
control plane applying its own "evidence must be real" rule to itself:

- `debug-assert-not-runtime-guard` — no dedicated unit test for the runtime-assert
  text predicate; validated via the pipeline integration test plus the
  `raw_pointer_alignment_debug_assert_only_not_guard` /
  `raw_pointer_bounds_debug_assert_only_not_guard` fixture-calibration entries.
- `owner-cards-grouped-not-hidden` — no dedicated unit test asserts owner cards
  stay in `cards.json` counts; the linked test covers `not_selected` presence and
  reason code only.

Each is a `proof_gap` carrying `owner` (core / analysis) + `review_after`
(2026-09-15), so the enforcing gate treats it as a tracked exception and the
review date forces a recheck.

## What was deferred — PR-7 (calibration de-bottleneck)

The lane plan's PR-7 — move calibration metadata into per-fixture `meta.toml`
with a generated `calibration.toml` aggregate, so parallel fixture PRs stop
colliding on the shared registry file — is **not** a single PR. A risk scout
found `policy/calibration.toml` is a ~6.6k-line hand-authored source of truth
that ~7 gate consumers treat as authoritative; a full per-fixture migration is
~617 new files plus a generator — a phased, 1–2 week effort, high-risk as a
big-bang. It is deferred to its own phased lane, tracked in **issue #1712**.
Recommended phases: (0) document the calibration consumers; (1) pilot `meta.toml`
on 3–5 fixtures behind a non-blocking validating generator, with
`calibration.toml` staying authoritative; (2) migrate one operation family; (3)
migrate in waves; (4) flip the aggregate to generated-only. No big-bang.

## Recommended next

- Close the two tracked stance proof-gaps with the dedicated unit tests they name
  (small; removes the last exceptions and lets those entries drop their
  `proof_gap`).
- Start the calibration de-bottleneck as its own phased lane (#1712) — do not
  fold it into another lane's tail.

## Lane PRs

PR-0 #1723 · PR-1 #1724 · PR-2 #1725 · PR-3 #1726 · PR-4 #1727 · PR-5 #1728 ·
PR-6 #1729 · PR-A #1731 · PR-B #1730 · PR-C #1732.
