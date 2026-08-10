# Dogfood report: 2026-08-10 serde holdout

Status: initial release-readiness holdout
Swarm base commit: `92850405`
Artifact status: local, untracked under `target/dogfood-work/`

This report records the first holdout run for `serde-holdout`. The target is
pinned to an exact commit and remains outside ordinary dogfood execution unless
the caller explicitly opts into holdout runs.

The analyzer was not tuned against this repository before recording the result.
No source edits or witness tools were used.

## Trust boundary

This is static unsafe contract review advisory evidence only; it makes no
support-tier promotion, calibration, policy, release, memory-safety, witness,
site-execution, or precision/recall claim. Its boundary markers are
`not memory-safety proof`, `not UB-free status`, `not Miri-clean status`,
`not site-execution proof`, and `not calibrated precision or recall`. The
result is diagnostic evidence for one pinned repository, not evidence of
ecosystem generalization or analyzer completeness on multi-crate workspaces.

## Scope

- Target: `serde-holdout`
- Repository: `serde-rs/serde`
- Commit: `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8`
- Commit date: 2026-07-18
- Commit subject: `Release 1.0.229`
- Partition: `holdout`
- Artifact: `target/dogfood-work/serde-holdout.unsafe-review.json`
- Artifact bytes: `38301`
- Artifact SHA-256: `73ab56566faa4477eeeb0bfdb1d6fd3e4f99e829074d64aa8405591ae27cab4f`
- Card cap: 50

`serde` was selected as the multi-crate workspace holdout dimension. The
repository is a workspace of `serde`, `serde_core`, `serde_derive`, and
`serde_test` plus a `test_suite`, with derive macro-generated code and a
modest real unsafe surface concentrated in `serde_core`. It exercises
cross-crate repository shape rather than the generated-code volume already
recorded by the rkyv holdout, and it was not selected for card count.

## Commands

```bash
test "$(gh api repos/serde-rs/serde/commits/7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8 --jq .sha)" = "7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8"
cargo run --locked -p xtask -- check-corpus-partitions
cargo run --locked -p xtask -- dogfood-exec --target serde-holdout --include-holdout --strict --clean --timeout 300
cargo run --locked -p xtask -- dogfood-exec --target serde-holdout --strict --timeout 30
```

Result:

- pinned commit: `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8`
- explicit holdout run: 1 ok / 0 failed
- elapsed time: 9.67 seconds for the recorded scan command after the pinned
  clone
- default execution check: rejected before cloning/scanning because
  `--include-holdout` was absent

## First result

| Target | Cards | Unsafe sites | Rust files | Contract missing | Guard missing | Unsafe unreached | Requires Loom | Miri unsupported | Unknown | Target feature | Status |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `serde-holdout` | 6 | 6 | 208 | 3 | 1 | 0 | 0 | 2 | 16.7% | 0.0% | `ok` |

Operation-family mix in the result was: `ffi` 2, `str_from_utf8_unchecked` 2,
`raw_pointer_read` 1, and `unknown` 1. The scan was not capped
(`scan_capped: false`): six cards is the complete unsafe surface found, not a
truncated sample.

The schema-valid result is intentionally a first diagnostic reading, not a
tuning target or an accuracy measurement. The 16.7% unknown classification is
a single uncategorized card out of six and is recorded as an observation, not
a precision claim or an analyzer defect. Any analyzer change motivated by this
target must start from a follow-up report or issue that names the observed
card shape, adds fixture or challenge coverage where needed, and preserves the
advisory boundary.

## Triage

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `serde-holdout` | `str::from_utf8_unchecked` in `serde_core/src/format.rs` and `serde_core/src/ser/impls.rs` | `actionable` | Two unchecked UTF-8 operations in workspace core crates; one is `contract_missing` (high confidence), one is `guard_missing` with a nearby guard (medium confidence). | Keep as holdout; any detector change needs a separate follow-up with fixture coverage. |
| `serde-holdout` | `raw_pointer_read` in `serde_core/src/ser/impls.rs` (`serialize`) | `actionable` | Single raw pointer read card with pointer-validity, alignment, initialized-memory, and same-allocation obligations inferred; classified `contract_missing`. | Preserve as first-result evidence; do not tune against it in this PR. |
| `serde-holdout` | `test_suite/no_std` extern block and `libc::abort` FFI call | `needs-verifier` | Two `ffi` family cards classified `miri_unsupported` in the no-std test-suite crate, showing the workspace scan reaches crates beyond the library core. | Record as workspace-shape evidence; no analyzer change in this PR. |
| `serde-holdout` | uncategorized `unsafe` block in `test_suite/no_std/src/main.rs` | `needs-verifier` | One of six cards (16.7%) emitted with an `unknown` operation family on a bare `unsafe` block in the panic handler. | Preserve this first result; add a focused fixture or challenge only after a separate follow-up decision. |
| `serde-holdout` | holdout execution | `needs-verifier` | The target is partitioned as holdout and the default command rejected execution before cloning or scanning. | Preserve the negative opt-in rail and do not tune directly against this first result. |

## Promotion path

Keep `serde-holdout` in the holdout partition through the next
release-readiness evaluation. After that evaluation records notable misses,
noise, setup friction, or card-shape follow-ups, the repository can either be
promoted to regression or rotated out for another fresh pinned target.

Do not tune directly against this holdout result before that follow-up decision
is recorded.
