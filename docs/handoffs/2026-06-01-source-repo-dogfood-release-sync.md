# 2026-06-01 source repo dogfood release sync

Status: source-to-swarm release-train acknowledgement

This handoff records the source repo-dogfood sync PRs that advanced
`EffortlessMetrics/unsafe-review` before the 0.3.2 publication gate.

Source PRs and commits:

| Source PR / commit | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#513` | `b67823bc` | Imported the reviewed swarm repo-dogfood batch with history preserved and added source release-gate hygiene | Mirrored by swarm PRs #1227/#1229 |
| `EffortlessMetrics/unsafe-review#514` | `8427bf31` | Imported the latest swarm dogfood fixture pin after #1226 | Acknowledged by this sync |

Swarm sync:

- Swarm PR #1227 mirrored the source release-gate hygiene tree.
- Swarm PR #1229 restored source history reachability with a history-only merge
  because #1227 was squash-merged by the normal swarm PR path.
- `policy/source-sync.toml` acknowledges source main at
  `8427bf3170cfb96afa2e60a3270169b0d3cab1f6`.

Publication status:

- `0.3.2` is prepared in source but not published by this sync.
- No `v0.3.2` tag is claimed here.
- No GitHub Release for `v0.3.2` is claimed here.
- crates.io still needs the explicit publish step from source.

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
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo run --locked -p xtask -- check-pr`
- `cargo run --locked -p xtask -- source-divergence`
- `git diff --check`

Expected after merge:

- `cargo run --locked -p xtask -- source-divergence` reports no raw
  source-only commits.
