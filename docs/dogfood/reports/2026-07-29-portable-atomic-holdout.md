# Dogfood report: 2026-07-29 portable-atomic holdout

Status: initial release-readiness holdout
Swarm base commit: `eba97c61`
Artifact status: local, untracked under `target/dogfood-work/`

This report records the first holdout run for `portable-atomic-holdout`. The
target is pinned to an exact commit and remains outside ordinary dogfood
execution unless the caller explicitly opts into holdout runs.

The analyzer was not tuned against this repository before recording the result.
No source edits or witness tools were used.

## Trust boundary

This is static unsafe contract review advisory evidence only. It is not a
support-tier promotion, calibration report, policy decision, release claim,
not a memory-safety proof, not UB-free status, not Miri-clean status, not a
witness result, not site-execution proof, or a calibrated precision or recall
figure. The capped
result is diagnostic evidence for one pinned repository, not evidence of
ecosystem generalization.

## Scope

- Target: `portable-atomic-holdout`
- Repository: `taiki-e/portable-atomic`
- Commit: `5716e951b47494703a3926403c27a20add27fa9b`
- Commit date: 2026-07-26
- Commit subject: `ci: Update FreeBSD 15 to 15.1, OpenBSD to 7.9`
- Partition: `holdout`
- Artifact: `target/dogfood-work/portable-atomic-holdout.unsafe-review.json`
- Artifact bytes: `274864`
- Artifact SHA-256: `f1066d09914033186c356b31aac6028b3c1ec0300d05bc00d29e21516eedfe70`
- Card cap: 50

`portable-atomic` was selected as a distinct `no_std` and atomic-state
holdout. It exercises platform-specific unsafe contracts, pointer state, and
atomic review routes not represented by the original RNG-focused holdout.

## Commands

```bash
gh api repos/taiki-e/portable-atomic/commits/HEAD --jq .sha
cargo run --locked -p xtask -- check-corpus-partitions
cargo run --locked -p xtask -- dogfood-exec --target portable-atomic-holdout --include-holdout --strict --clean --timeout 300
cargo run --locked -p xtask -- dogfood-exec --target portable-atomic-holdout --strict --timeout 30
```

Result:

- pinned commit: `5716e951b47494703a3926403c27a20add27fa9b`
- explicit holdout run: 1 ok / 0 failed
- elapsed time: 19.41 seconds
- default execution check: rejected before cloning/scanning because
  `--include-holdout` was absent

## First result

| Target | Cards | Unsafe sites | Rust files | Contract missing | Guard missing | Requires Loom | Miri unsupported | Unknown | Target feature | Status |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `portable-atomic-holdout` | 50 | 965 | 67 | 26 | 15 | 5 | 4 | 14.0% | 2.0% | `ok` |

Operation-family mix in the capped result was: `unsafe_declaration` 16,
`inline_asm` 10, `unknown` 7, `unsafe_impl_send_sync` 5, `ffi` 4,
`unsafe_fn_call` 3, and one each for `target_feature`,
`slice_from_raw_parts`, `raw_pointer_write`, `raw_pointer_deref`, and
`pointer_arithmetic`.

The schema-valid result is intentionally a first diagnostic reading, not a
tuning target or an accuracy measurement. Any analyzer change motivated by
this target must start from a follow-up report or issue that names the observed
card shape, adds fixture or challenge coverage where needed, and preserves the
advisory boundary.

## Triage

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `portable-atomic-holdout` | no-std atomic, pointer, and platform review routes | `actionable` | First holdout run completed with a capped, schema-valid repo snapshot on a pinned external crate. | Keep as holdout until a release-readiness decision records whether to promote it to regression or rotate it. |
| `portable-atomic-holdout` | holdout execution | `needs-verifier` | The target is partitioned as holdout and the default command rejected execution before cloning or scanning. | Preserve the negative opt-in rail and do not tune directly against this first result. |

## Promotion path

Keep `portable-atomic-holdout` in the holdout partition through the next
release-readiness evaluation. After that evaluation records notable misses,
noise, setup friction, or card-shape follow-ups, the repository can either be
promoted to regression or rotated out for another fresh pinned target.

Do not tune directly against this holdout result before that follow-up decision
is recorded.
