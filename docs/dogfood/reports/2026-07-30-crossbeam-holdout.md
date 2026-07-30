# Dogfood report: 2026-07-30 crossbeam holdout

Status: initial release-readiness holdout
Swarm base commit: `bafcde9c`
Artifact status: local, untracked under `target/dogfood-work/`

This report records the first holdout run for `crossbeam-holdout`. The target
is pinned to an exact commit and remains outside ordinary dogfood execution
unless the caller explicitly opts into holdout runs.

The analyzer was not tuned against this repository before recording the result.
No source edits or witness tools were used.

## Trust boundary

This is static unsafe contract review advisory evidence only; it makes no
support-tier promotion, calibration, policy, release, memory-safety, witness,
site-execution, or precision/recall claim. Its boundary markers are
`not memory-safety proof`, `not UB-free status`, `not Miri-clean status`,
`not site-execution proof`, and `not calibrated precision or recall`. The
capped result is diagnostic evidence for one pinned repository, not evidence
of ecosystem generalization.

## Scope

- Target: `crossbeam-holdout`
- Repository: `crossbeam-rs/crossbeam`
- Commit: `b23b7e8eca2efdad9bdc1ceb1aee1207a852c03b`
- Commit date: 2026-07-12
- Commit subject: `Replace all transmute_copy uses with safer transmute_copy_by_val`
- Partition: `holdout`
- Artifact: `target/dogfood-work/crossbeam-holdout.unsafe-review.json`
- Artifact bytes: `308052`
- Artifact SHA-256: `2f37815d36207e9a9410e2c296b4b47f18e0ca5ce1817720c031449defa35555`
- Card cap: 50

`crossbeam` was selected as a distinct concurrency and ownership holdout. Its
workspace exercises atomics, unsafe Send/Sync implementations, strict-
provenance pointer state, ownership transfer, and cross-crate unsafe routes
not represented by the RNG, no_std atomic, or SIMD holdouts.

## Commands

```bash
test "$(gh api repos/crossbeam-rs/crossbeam/commits/b23b7e8eca2efdad9bdc1ceb1aee1207a852c03b --jq .sha)" = "b23b7e8eca2efdad9bdc1ceb1aee1207a852c03b"
cargo run --locked -p xtask -- check-corpus-partitions
cargo run --locked -p xtask -- dogfood-exec --target crossbeam-holdout --include-holdout --strict --clean --timeout 300
cargo run --locked -p xtask -- dogfood-exec --target crossbeam-holdout --strict --timeout 30
```

Result:

- pinned commit: `b23b7e8eca2efdad9bdc1ceb1aee1207a852c03b`
- explicit holdout run: 1 ok / 0 failed
- elapsed time: 24.02 seconds
- default execution check: rejected before cloning/scanning because
  `--include-holdout` was absent

## First result

| Target | Cards | Unsafe sites | Rust files | Contract missing | Guard missing | Requires Loom | Miri unsupported | Unknown | Target feature | Status |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `crossbeam-holdout` | 50 | 693 | 114 | 47 | 1 | 2 | 0 | 2.0% | 0.0% | `ok` |

Operation-family mix in the capped result was: `unsafe_fn_call` 18,
`unsafe_declaration` 10, `raw_pointer_read` 9, `raw_pointer_deref` 3,
`get_unchecked` 2, `pointer_arithmetic` 2,
`unsafe_impl_send_sync` 2, `box_from_raw` 1, `raw_pointer_write` 1,
`transmute` 1, and one uncategorized card.

The schema-valid result is intentionally a first diagnostic reading, not a
tuning target or an accuracy measurement. Any analyzer change motivated by
this target must start from a follow-up report or issue that names the observed
card shape, adds fixture or challenge coverage where needed, and preserves the
advisory boundary.

## Triage

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `crossbeam-holdout` | concurrency, atomics, pointer, and ownership-transfer routes | `actionable` | First holdout run completed with a capped, schema-valid repo snapshot on a pinned external workspace. | Keep as holdout until a release-readiness decision records whether to promote it to regression or rotate it. |
| `crossbeam-holdout` | holdout execution | `needs-verifier` | The target is partitioned as holdout and the default command rejected execution before cloning or scanning. | Preserve the negative opt-in rail and do not tune directly against this first result. |

## Promotion path

Keep `crossbeam-holdout` in the holdout partition through the next
release-readiness evaluation. After that evaluation records notable misses,
noise, setup friction, or card-shape follow-ups, the repository can either be
promoted to regression or rotated out for another fresh pinned target.

Do not tune directly against this holdout result before that follow-up decision
is recorded.
