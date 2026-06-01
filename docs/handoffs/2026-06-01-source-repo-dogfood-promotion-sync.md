# 2026-06-01 source repo dogfood promotion sync

Status: source-to-swarm promotion acknowledgement

This handoff records `EffortlessMetrics/unsafe-review#511`, which imported the
reviewed swarm repo-dogfood usability batch into the public source repository
with history preserved. It also mirrors the source-only clippy cleanups that
were made while validating that source promotion.

Source PRs and commits:

| Source PR / commit | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#511` | `01959b09` | Imported the swarm repo-dogfood batch with a merge commit | Acknowledged by this sync |
| source sync branch merge | `0b186248` | Kept the reviewed swarm commits reachable and applied source validation fixes | Source-only fixes mirrored here |

Swarm sync:

- `crates/unsafe-review-cli/src/execute.rs` mirrors the source poisoned-lock
  error messages for repo status and partial-output state.
- `xtask/src/accuracy_labels.rs` mirrors the source label-ledger validation
  context split used to satisfy strict clippy.
- `policy/source-sync.toml` acknowledges source main at
  `01959b0945e76e75167715cea8992dc0b119a632`.
- Swarm-only commit `f68d23d2` remains workbench-only and unpromoted to source.

Publication status:

- `0.3.2` is prepared in source but not published by this sync.
- No `v0.3.2` tag is claimed here.
- No GitHub Release for `v0.3.2` is claimed here.

Boundaries:

- no publication claim
- no tag or GitHub Release claim
- no Bun vulnerability claim
- no witness execution
- no automatic comments
- no source edits by `unsafe-review`
- no default blocking policy
- no safety, UB-free, Miri-clean, site-execution, precision, recall, or
  policy-readiness claim

Validation:

- `cargo fmt --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo run --locked -p xtask -- check-pr`
- `cargo run --locked -p xtask -- source-divergence`
- `git diff --check`

Expected after merge:

- `cargo run --locked -p xtask -- source-divergence` reports
  `new_source_commits=0`.
