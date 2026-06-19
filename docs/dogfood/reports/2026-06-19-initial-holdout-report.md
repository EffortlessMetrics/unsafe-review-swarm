# Dogfood report: 2026-06-19 initial holdout report

Status: initial release-readiness holdout
Swarm base commit: `e05fafeb`
Artifact status: local, untracked under `target/dogfood-work/`

This report records the first holdout run for `getrandom-holdout`, a
release-readiness repo-snapshot target added after the conformance/regression
partition contract landed. The target is pinned to an exact commit and remains
outside ordinary dogfood execution unless the caller explicitly opts into
holdout runs.

The analyzer was not tuned against this repository before recording the result
below. The only pre-run code change in this PR was the execution rail that keeps
holdout targets out of default `dogfood-exec` runs.

## Trust boundary

This is static unsafe contract review advisory evidence only. It is not a
support-tier promotion, not a calibration report, not a policy decision, not a
release claim, not a memory-safety proof, not UB-free status, not Miri-clean
status, not a witness result, not site-execution proof, and not a calibrated
precision or recall figure. No witness tools were run. The capped holdout result
is diagnostic release-readiness telemetry for one pinned repository, not
evidence of ecosystem generalization.

## Scope

- Target: `getrandom-holdout`
- Repository: `rust-random/getrandom`
- Commit: `5e7cd5733536844a9856dc7259bd4696bbe5e3ae`
- Commit date: 2026-06-17
- Commit subject: `Release v0.4.3 (#853)`
- Partition: `holdout`
- Artifact: `target/dogfood-work/getrandom-holdout.unsafe-review.json`
- Card cap: 50

`getrandom` was selected as a small external holdout with platform RNG unsafe
declarations, FFI-style bindings, raw pointer operations, and target-feature
surface area. It was not part of the existing regression corpus.

## Commands

```bash
rtk gh api repos/rust-random/getrandom/commits/HEAD --jq .sha
rtk cargo run --locked -p xtask -- dogfood-exec --target getrandom-holdout --include-holdout --strict --clean --timeout 300
rtk cargo run --locked -p xtask -- dogfood-exec --target getrandom-holdout --strict --timeout 30
```

Result:

- pinned HEAD at selection time:
  `5e7cd5733536844a9856dc7259bd4696bbe5e3ae`
- explicit holdout run: 1 ok / 0 failed
- negative opt-in check: failed before cloning/scanning with
  `rerun with --include-holdout`

## First Result

| Target | Cards | Summary cards | Unsafe sites | Rust files | Unknown | Target feature | Status |
|---|---:|---:|---:|---:|---:|---:|---|
| `getrandom-holdout` | 50 | 50 | 139 | 41 | 2.0% | 8.0% | `ok` |

The capped scan produced schema-valid JSON. The result is intentionally a first
diagnostic reading, not a tuning target. Any analyzer change motivated by this
target must start from a follow-up report or issue that names the observed card
shape, adds fixture or challenge coverage where needed, and preserves the
advisory boundary.

## Triage

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `getrandom-holdout` | `repo-snapshot` | `actionable` | First holdout run completed with a capped, schema-valid repo snapshot on a previously unused external crate. | Keep as holdout until a release-readiness decision records whether to promote it to regression. |
| `getrandom-holdout` | `holdout execution` | `needs-verifier` | A targeted `dogfood-exec` run without `--include-holdout` fails before cloning/scanning, preventing accidental tuning runs. | Covered by `dogfood_exec` tests and the negative command above. |

## Promotion Path

Keep `getrandom-holdout` in the holdout partition through the next
release-readiness evaluation. After that evaluation records any notable misses,
noise, setup friction, or card-shape follow-ups, the repo can either:

- promote the target to regression by removing the per-target
  `partition = "holdout"` override, because `repo-snapshot` defaults to
  regression; or
- rotate it out of holdout and replace it with another fresh pinned target.

Do not tune directly against this holdout result before that follow-up decision
is recorded.
