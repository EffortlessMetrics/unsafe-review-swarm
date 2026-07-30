# Dogfood report: 2026-07-30 rkyv holdout

Status: initial release-readiness holdout
Swarm base commit: `67ccb873`
Artifact status: local, untracked under `target/dogfood-work/`

This report records the first holdout run for `rkyv-holdout`. The target is
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
capped result is diagnostic evidence for one pinned repository, not evidence
of ecosystem generalization.

## Scope

- Target: `rkyv-holdout`
- Repository: `rkyv/rkyv`
- Commit: `46e143d6e4c8c5a5f4ce5e19a24201e577e5706d`
- Commit date: 2026-07-02
- Commit subject: `Release 0.8.17`
- Partition: `holdout`
- Artifact: `target/dogfood-work/rkyv-holdout.unsafe-review.json`
- Artifact bytes: `272800`
- Artifact SHA-256: `9bfe809000fcd1c1787cebec864f31407d097ccc37deb5cf11fb74fc4d6f7a19`
- Card cap: 50

`rkyv` was selected as a distinct macro-heavy and generated-code holdout. Its
multi-crate workspace exercises allocation, raw pointers, byte casts,
unchecked operations, and generated unsafe routes not represented by the RNG,
no_std atomic, SIMD, or concurrency holdouts.

## Commands

```bash
test "$(gh api repos/rkyv/rkyv/commits/46e143d6e4c8c5a5f4ce5e19a24201e577e5706d --jq .sha)" = "46e143d6e4c8c5a5f4ce5e19a24201e577e5706d"
cargo run --locked -p xtask -- check-corpus-partitions
cargo run --locked -p xtask -- dogfood-exec --target rkyv-holdout --include-holdout --strict --clean --timeout 300
cargo run --locked -p xtask -- dogfood-exec --target rkyv-holdout --strict --timeout 30
```

Result:

- pinned commit: `46e143d6e4c8c5a5f4ce5e19a24201e577e5706d`
- explicit holdout run: 1 ok / 0 failed
- elapsed time: 31.47 seconds
- default execution check: rejected before cloning/scanning because
  `--include-holdout` was absent

## First result

| Target | Cards | Unsafe sites | Rust files | Contract missing | Guard missing | Unsafe unreached | Requires Loom | Miri unsupported | Unknown | Target feature | Status |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `rkyv-holdout` | 50 | 930 | 168 | 46 | 3 | 1 | 0 | 0 | 16.0% | 0.0% | `ok` |

Operation-family mix in the capped result was: `unsafe_fn_call` 28,
`unsafe_declaration` 7, `pointer_arithmetic` 1, `raw_pointer_deref` 1,
`raw_pointer_read` 1, `raw_pointer_read_unaligned` 1,
`raw_pointer_write` 1, `slice_from_raw_parts` 1,
`unreachable_unchecked` 1, and 8 uncategorized cards.

The schema-valid result is intentionally a first diagnostic reading, not a
tuning target or an accuracy measurement. The 16.0% unknown classification is
recorded as an observation, not a precision claim or an analyzer defect. Any
analyzer change motivated by this target must start from a follow-up report or
issue that names the observed card shape, adds fixture or challenge coverage
where needed, and preserves the advisory boundary.

## Triage

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `rkyv-holdout` | macro-generated unsafe calls, allocation, pointer, and byte-cast routes | `actionable` | First holdout run completed with a capped, schema-valid repo snapshot on a pinned external workspace. | Keep as holdout until a release-readiness decision records whether to promote it to regression or rotate it. |
| `rkyv-holdout` | unknown operation-family classification | `needs-verifier` | Eight capped cards were emitted with an uncategorized operation family in a macro-heavy workspace. | Preserve this first result; add a focused fixture or challenge only after a separate follow-up decision. |
| `rkyv-holdout` | holdout execution | `needs-verifier` | The target is partitioned as holdout and the default command rejected execution before cloning or scanning. | Preserve the negative opt-in rail and do not tune directly against this first result. |

## Promotion path

Keep `rkyv-holdout` in the holdout partition through the next release-readiness
evaluation. After that evaluation records notable misses, noise, setup
friction, or card-shape follow-ups, the repository can either be promoted to
regression or rotated out for another fresh pinned target.

Do not tune directly against this holdout result before that follow-up decision
is recorded.
