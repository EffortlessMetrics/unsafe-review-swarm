# Dogfood report: 2026-08-14 holdout rotation closeout

Status: rotation decision (issue #1891)
Swarm base commit: `669c1d71ec408b7599a2239b1843f221da3698fd`
Artifact status: docs-only decision; no holdout target was cloned, scanned, rerun, tuned, promoted, replaced, or added

This report closes the initial seven-target holdout expansion cycle. It records
one bounded rotation decision against the authoritative target definitions in
`docs/dogfood/corpus.toml` and the immutable first-result reports linked below:
retain all seven targets as frozen holdouts and promote none to regression.

The decision implements the refresh policy in
`docs/specs/UNSAFE-REVIEW-SPEC-0042-corpus-validation-taxonomy.md`. It does not
create a second corpus manifest, change the generated dogfood indexes, or
rewrite any first-result evidence.

## Trust boundary

Holdout partition membership and first-result reports are diagnostic static
unsafe-review evidence on seven selected pinned repositories. This closeout is
not a witness result, memory-safety proof, safe or UB-free claim, Miri-clean
status, site-execution proof, calibrated precision or recall measurement,
policy decision, support-tier promotion, release-readiness result, or evidence
about repositories outside the recorded set. No witness tools were run.

## Audited inventory and decision

The seven entries below remain `status = "active"` and
`partition = "holdout"` in `docs/dogfood/corpus.toml`. Their exact target SHAs,
first-result reports, and recorded artifact hashes remain unchanged.

| Target | Exact target SHA | Distinct dimension | Retained first-result report | Recorded artifact SHA-256 | Decision |
|---|---|---|---|---|---|
| `getrandom-holdout` | `5e7cd5733536844a9856dc7259bd4696bbe5e3ae` | platform RNG, FFI-style bindings, raw pointers | [`2026-06-19-initial-holdout-report.md`](2026-06-19-initial-holdout-report.md) | Not recorded by the original report; not reconstructed or rerun here | retain frozen holdout |
| `portable-atomic-holdout` | `5716e951b47494703a3926403c27a20add27fa9b` | `no_std` atomics, pointer state, platform contracts | [`2026-07-29-portable-atomic-holdout.md`](2026-07-29-portable-atomic-holdout.md) | `f1066d09914033186c356b31aac6028b3c1ec0300d05bc00d29e21516eedfe70` | retain frozen holdout |
| `simdutf8-holdout` | `641d57f313df57354246d2b68d4778c092e076c3` | SIMD, `target_feature`, intrinsics, unchecked UTF-8 | [`2026-07-29-simdutf8-holdout.md`](2026-07-29-simdutf8-holdout.md) | `25a32a1c20ed2b500c77780c86cb36637eecb358d8278a9012d96eb0252fa596` | retain frozen holdout |
| `crossbeam-holdout` | `b23b7e8eca2efdad9bdc1ceb1aee1207a852c03b` | concurrency, atomics, strict-provenance pointers, ownership transfer | [`2026-07-30-crossbeam-holdout.md`](2026-07-30-crossbeam-holdout.md) | `2f37815d36207e9a9410e2c296b4b47f18e0ca5ce1817720c031449defa35555` | retain frozen holdout |
| `rkyv-holdout` | `46e143d6e4c8c5a5f4ce5e19a24201e577e5706d` | macro-generated code, allocation, raw pointers, byte casts | [`2026-07-30-rkyv-holdout.md`](2026-07-30-rkyv-holdout.md) | `9bfe809000fcd1c1787cebec864f31407d097ccc37deb5cf11fb74fc4d6f7a19` | retain frozen holdout |
| `slab-holdout` | `a1e4346070a48c936d808de75191dee5d01e433c` | quiet unsafe surface, pointer arithmetic, unsafe implementations | [`2026-07-30-slab-holdout.md`](2026-07-30-slab-holdout.md) | `a1e8869a804c47d2871a81789246b9a394289b043c6cdb5a52f0220dc36aeb91` | retain frozen holdout |
| `serde-holdout` | `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8` | multi-crate workspace, derive-generated code, modest core unsafe surface | [`2026-08-10-serde-holdout.md`](2026-08-10-serde-holdout.md) | `73ab56566faa4477eeeb0bfdb1d6fd3e4f99e829074d64aa8405591ae27cab4f` | retain frozen holdout |

No target is promoted because this cycle establishes breadth and preserves the
first untuned observation for each dimension; it does not establish a stable,
repeatable regression expectation for any target. Several reports are capped,
and their selected card counts and classifications remain diagnostic rather
than exact goldens. Promotion now would silently turn fresh-input evidence into
ordinary tuning input without the separate follow-up decision required by
SPEC-0042.

## Next evaluation cycle

The next cycle begins only through an explicit issue or cutline item that names
the evaluation purpose and current tool commit. It is not calendar-scheduled
and is not triggered by an ordinary PR, `check-pr`, or a detector change.

At that cycle:

1. Audit the seven retained definitions and reports before executing any
   target.
2. Select at least one fresh, suitable, exact-SHA replacement when one is
   available. The replacement must add a distinct or materially refreshed
   repository shape rather than maximize card count.
3. Record the replacement target's first result in a new committed report
   before any tuning, detector, ranking, or surfacing change uses the result.
4. Retain the rotated target's historic definition in git history and retain
   its first-result report in this directory for comparison.
5. If no suitable fresh replacement is available, record that bounded
   deferral; do not substitute a floating ref or silently rerun a familiar
   target as though it were fresh.

## Promotion and replacement rules

A future PR may promote one target to regression only when it records all of:

- the retained first-result report;
- a follow-up decision explaining why the target is now a stable regression
  input rather than unseen holdout evidence;
- the bounded behavior or invariant that regression execution should protect;
- the exact manifest change and the normal regression cadence; and
- `check-corpus-partitions`, `check-dogfood`, and the relevant execution proof.

Rotation without promotion must replace the target with a fresh exact-SHA
entry in the same bounded lane or explicitly record why replacement is
deferred. Neither route may delete or rewrite the original report, turn a
holdout into every-PR execution, or tune directly against an unrecorded first
result.

## Opt-in negative control

The closeout rechecks the execution boundary with a targeted command that omits
`--include-holdout`:

```text
rtk cargo run --locked -p xtask -- dogfood-exec --target serde-holdout --strict --timeout 30
```

Expected and required result: nonzero exit before clone or scan, with guidance
to rerun using `--include-holdout`. This is a negative control, not a holdout
execution and not a new first result.

## Decision

The initial matrix is complete at seven exact-SHA targets across seven distinct
dimensions. All seven remain frozen holdouts; none is promoted, replaced, or
rerun in this closeout. The next rotation remains an explicit future evaluation
cycle under SPEC-0042, with original reports and hashes retained as the audit
record.
