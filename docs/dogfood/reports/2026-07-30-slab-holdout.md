# Dogfood report: 2026-07-30 slab holdout

Status: initial release-readiness holdout
Swarm base commit: `49953696`
Artifact status: local, untracked under `target/dogfood-work/`

This report records the first holdout run for `slab-holdout`. The target is
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
of ecosystem generalization or analyzer completeness on quiet repositories.

## Scope

- Target: `slab-holdout`
- Repository: `tokio-rs/slab`
- Commit: `a1e4346070a48c936d808de75191dee5d01e433c`
- Commit date: 2026-01-31
- Commit subject: `Release v0.4.12 (#161)`
- Partition: `holdout`
- Artifact: `target/dogfood-work/slab-holdout.unsafe-review.json`
- Artifact bytes: `61719`
- Artifact SHA-256: `a1e8869a804c47d2871a81789246b9a394289b043c6cdb5a52f0220dc36aeb91`
- Card cap: 50

`slab` was selected as a comparatively quiet and small unsafe-surface
holdout. Its five Rust files exercise pointer arithmetic, unchecked access,
`MaybeUninit` initialization, and unsafe declarations without selecting a
target merely for a large card count.

## Commands

```bash
test "$(gh api repos/tokio-rs/slab/commits/a1e4346070a48c936d808de75191dee5d01e433c --jq .sha)" = "a1e4346070a48c936d808de75191dee5d01e433c"
cargo run --locked -p xtask -- check-corpus-partitions
cargo run --locked -p xtask -- dogfood-exec --target slab-holdout --include-holdout --strict --clean --timeout 300
cargo run --locked -p xtask -- dogfood-exec --target slab-holdout --strict --timeout 30
```

Result:

- pinned commit: `a1e4346070a48c936d808de75191dee5d01e433c`
- explicit holdout run: 1 ok / 0 failed
- elapsed time: 1.52 seconds
- default execution check: rejected before cloning/scanning because
  `--include-holdout` was absent

## First result

| Target | Cards | Unsafe sites | Rust files | Contract missing | Guard missing | Unsafe unreached | Requires Loom | Miri unsupported | Unknown | Target feature | Status |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `slab-holdout` | 11 | 11 | 5 | 0 | 8 | 3 | 0 | 0 | 0.0% | 0.0% | `ok` |

Operation-family mix in the result was: `pointer_arithmetic` 4,
`unsafe_declaration` 3, `get_unchecked` 2, `maybe_uninit_assume_init` 1,
and `raw_pointer_deref` 1.

The result is intentionally a first diagnostic reading, not a tuning target or
an accuracy measurement. The small card count is a corpus-shape observation,
not evidence of analyzer completeness or quiet-repository safety. Any analyzer
change motivated by this target must start from a follow-up report or issue
that names the observed card shape, adds fixture or challenge coverage where
needed, and preserves the advisory boundary.

## Triage

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `slab-holdout` | pointer arithmetic, unchecked access, and initialization routes | `actionable` | First holdout run completed with a schema-valid repo snapshot on a pinned external crate. | Keep as holdout until a release-readiness decision records whether to promote it to regression or rotate it. |
| `slab-holdout` | quiet-surface coverage | `needs-verifier` | The target produced 11 cards across 5 Rust files, providing a small-surface comparison point. | Preserve as a diagnostic quiet-surface holdout; do not infer completeness from the low card count. |
| `slab-holdout` | holdout execution | `needs-verifier` | The target is partitioned as holdout and the default command rejected execution before cloning or scanning. | Preserve the negative opt-in rail and do not tune directly against this first result. |

## Promotion path

Keep `slab-holdout` in the holdout partition through the next release-readiness
evaluation. After that evaluation records notable misses, noise, setup
friction, or card-shape follow-ups, the repository can either be promoted to
regression or rotated out for another fresh pinned target.

Do not tune directly against this holdout result before that follow-up decision
is recorded.
