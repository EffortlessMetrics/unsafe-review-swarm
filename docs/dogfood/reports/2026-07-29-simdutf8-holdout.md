# Dogfood report: 2026-07-29 simdutf8 holdout

Status: initial release-readiness holdout
Swarm base commit: `4ac19f02`
Artifact status: local, untracked under `target/dogfood-work/`

This report records the first holdout run for `simdutf8-holdout`. The target is
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

- Target: `simdutf8-holdout`
- Repository: `rusticstuff/simdutf8`
- Commit: `641d57f313df57354246d2b68d4778c092e076c3`
- Commit date: 2026-06-15
- Commit subject: `AVX-512 micro optimizations for non-ASCII input (#128)`
- Partition: `holdout`
- Artifact: `target/dogfood-work/simdutf8-holdout.unsafe-review.json`
- Artifact bytes: `285599`
- Artifact SHA-256: `25a32a1c20ed2b500c77780c86cb36637eecb358d8278a9012d96eb0252fa596`
- Card cap: 50

`simdutf8` was selected as a distinct SIMD and `target_feature` holdout. Its
AVX-512 implementation exercises intrinsic, target-feature, and unchecked
UTF-8 review routes not represented by the RNG or atomic holdouts.

## Commands

```bash
gh api repos/rusticstuff/simdutf8/commits/HEAD --jq .sha
cargo run --locked -p xtask -- check-corpus-partitions
cargo run --locked -p xtask -- dogfood-exec --target simdutf8-holdout --include-holdout --strict --clean --timeout 300
cargo run --locked -p xtask -- dogfood-exec --target simdutf8-holdout --strict --timeout 30
```

Result:

- pinned commit: `641d57f313df57354246d2b68d4778c092e076c3`
- explicit holdout run: 1 ok / 0 failed
- elapsed time: 36.30 seconds
- default execution check: rejected before cloning/scanning because
  `--include-holdout` was absent

## First result

| Target | Cards | Unsafe sites | Rust files | Contract missing | Guard missing | Requires Loom | Miri unsupported | Unknown | Target feature | Status |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `simdutf8-holdout` | 50 | 343 | 47 | 49 | 0 | 0 | 1 | 0.0% | 12.0% | `ok` |

Operation-family mix in the capped result was: `unsafe_declaration` 34,
`unsafe_fn_call` 8, `target_feature` 6, `transmute` 1, and `ffi` 1.

The schema-valid result is intentionally a first diagnostic reading, not a
tuning target or an accuracy measurement. Any analyzer change motivated by
this target must start from a follow-up report or issue that names the observed
card shape, adds fixture or challenge coverage where needed, and preserves the
advisory boundary.

## Triage

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `simdutf8-holdout` | SIMD, target-feature, and intrinsic review routes | `actionable` | First holdout run completed with a capped, schema-valid repo snapshot on a pinned external crate. | Keep as holdout until a release-readiness decision records whether to promote it to regression or rotate it. |
| `simdutf8-holdout` | holdout execution | `needs-verifier` | The target is partitioned as holdout and the default command rejected execution before cloning or scanning. | Preserve the negative opt-in rail and do not tune directly against this first result. |

## Promotion path

Keep `simdutf8-holdout` in the holdout partition through the next
release-readiness evaluation. After that evaluation records notable misses,
noise, setup friction, or card-shape follow-ups, the repository can either be
promoted to regression or rotated out for another fresh pinned target.

Do not tune directly against this holdout result before that follow-up decision
is recorded.
