# 2026-06-05 source workbench docs and readiness sync

Status: source-to-swarm history checkpoint sync

This handoff records source PR #525 and advances the swarm source-sync
checkpoint after source performed a history-preserving catch-up from swarm.
This is not a release, analyzer expansion, Bun finding, or policy-gate
promotion.

Source PRs and commits:

| Source PR / commit | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#525` | `9cc43b64` | Preserved reviewed swarm workbench docs and readiness contracts plus source CI budget acknowledgement | Acknowledged by this sync as source history and validation state |

Source PR #525 route:

- Source branch: `sync/source-workbench-2026-06-05`.
- Source merge commit:
  `9cc43b640b4d9618919d7e1e6cbde72ae83a5822`.
- History-preserving catch-up merge:
  `f36bf8bb2f7bc49b986d14b677219fad68c1530b`.
- Source PR body source swarm main:
  `0c89380926e1e4bbad69c528b3a63aa22e224499`.
- Current swarm-only workbench commits preserved for later promotion:
  `0a997f29a9086893f3db4154dd289f31a05f1d26` and
  `3c08c03f0af1c8a9bde95654596411ad9cfbc75c`.

Swarm sync:

- `unsafe-review-swarm` acknowledges source main at
  `9cc43b640b4d9618919d7e1e6cbde72ae83a5822`.
- `policy/source-sync.toml` advances the source checkpoint to that source main.
- The current swarm tree remains ahead of source by the node-parity oracle-map
  slice from swarm PR #1497 and the tokmd stop-line verifier slice from swarm
  PR #1498. That work stays swarm-only until deliberately promoted to source.
- A tree comparison of `source/main..origin/main` before this sync showed only
  #1497/#1498's five-file delta:
  `crates/unsafe-review/tests/e2e.rs`,
  `docs/examples/manual-candidates/candidate7-sync-compression-getter-reentry.json`,
  `docs/examples/manual-candidates/zstd-overlap.json`,
  `xtask/src/advisory_artifacts.rs`, and `xtask/src/main.rs`.
- The swarm repository disallows PR merge commits, so the final swarm commit may
  be a squash merge rather than an ancestry-preserving merge of source main.
  The source-sync checkpoint is the authority for whether source has moved since
  the last acknowledged absorption.

Boundaries:

- no crates.io publication claim
- no tag or GitHub Release claim
- no analyzer breadth beyond source-imported reviewed swarm work
- no Bun finding or vulnerability claim
- no witness execution
- no automatic comments
- no source edits by `unsafe-review`
- no default blocking policy
- no safety, UB-free, Miri-clean, site-execution, precision, recall, or
  policy-readiness claim

Validation:

- `cargo run --locked -p xtask -- source-divergence`
- `cargo run --locked -p xtask -- check-docs`
- `cargo run --locked -p xtask -- check-doc-artifacts`
- `cargo run --locked -p xtask -- check-goals`
- `cargo run --locked -p xtask -- check-pr`
- `git diff --check`

Expected after merge:

- `cargo run --locked -p xtask -- source-divergence` reports
  `new_source_commits=0`.
- Raw ancestry-only source/swarm divergence may remain nonzero under the
  repository's squash-merge policy.
