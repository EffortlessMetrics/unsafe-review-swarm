# 2026-05-26 source Dependabot sync

Status: source-to-swarm dependency sync

This handoff records two direct source dependency maintenance PRs and their
swarm mirror. This is not a routine source implementation lane.

Source PRs:

| Source PR | Source commit | Dependency change | Source status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#490` | `9312ff6` | `serde_json` `1.0.149` -> `1.0.150` | merged |
| `EffortlessMetrics/unsafe-review#491` | `85933ef` | `ra_ap_syntax` `0.0.333` -> `0.0.334` | merged |

Swarm mirror:

- `crates/unsafe-review-core/Cargo.toml` now depends on `ra_ap_syntax = "0.0.334"`.
- `Cargo.lock` now records:
  - `serde_json` `1.0.150`
  - `ra_ap_edition` `0.0.334`
  - `ra_ap_parser` `0.0.334`
  - `ra_ap_stdx` `0.0.334`
  - `ra_ap_syntax` `0.0.334`

Source-sync checkpoint:

- `policy/source-sync.toml` acknowledges source main at
  `85933ef284bfea31904fa73b78cae3c8df2b3996`.

Validation:

- `cargo fmt --check`
- `cargo check --workspace --all-targets --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo run --locked -p xtask -- check-pr`
- `git diff --check`

Expected after merge:

- `cargo run --locked -p xtask -- source-divergence` reports
  `new_source_commits=0`.

Boundaries:

- no analyzer behavior change
- no source promotion from swarm
- no witness execution
- no automatic comments
- no source edits by `unsafe-review`
- no default blocking policy
- no safety, UB-free, Miri-clean, site-execution, precision, or recall claim
