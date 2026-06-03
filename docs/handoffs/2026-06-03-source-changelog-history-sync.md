# 2026-06-03 source changelog and manual-candidate history sync

Status: source-to-swarm history checkpoint sync

This handoff records source PRs #521 and #522 and advances the swarm
source-sync checkpoint after source performed history-preserving catch-ups from
swarm. This is not a release, analyzer expansion, Bun finding, or policy-gate
promotion.

Source PRs and commits:

| Source PR / commit | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#521` | `c9fb2c90` | Preserved reviewed swarm changelog/usability history and source CI budget acknowledgement | Acknowledged by this sync as source history and validation state |
| `EffortlessMetrics/unsafe-review#522` | `7d87fa78` | Preserved reviewed swarm manual-candidate handoff cross-check history and source CI budget acknowledgement | Merged into swarm by this sync as source history and validation state |

Source PR #521 route:

- Source branch: `sync/usability-changelog`.
- Source merge commit:
  `c9fb2c905312759607ef27ec5380b25a4a4a46cd`.
- History-preserving catch-up merge:
  `03cb314c`.
- Source CI rerun acknowledgement commit:
  `b5f6cdd1`.

Source PR #522 route:

- Source branch: `sync/manual-candidate-handoff-crosscheck`.
- Source merge commit:
  `7d87fa782e8c9fd333d2c2436ec917207f0cd5c2`.
- History-preserving catch-up merge:
  `b51ac178`.
- Source CI rerun acknowledgement commit:
  `0de07a14`.

Swarm sync:

- `unsafe-review-swarm` absorbs the source main checkpoint from
  `7d87fa782e8c9fd333d2c2436ec917207f0cd5c2`.
- `policy/source-sync.toml` acknowledges source main at
  `7d87fa782e8c9fd333d2c2436ec917207f0cd5c2`.
- Source main `7d87fa78` contains swarm main `8b9582b9`, and the source and
  swarm trees have no diff at this checkpoint.
- The swarm repository disallows PR merge commits, so the final swarm commit may
  be a squash merge rather than an ancestry-preserving merge of source main.
  The source-sync checkpoint is the authority for whether source has moved
  since the last acknowledged absorption.

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

- `cargo run --locked -p xtask -- check-docs`
- `cargo run --locked -p xtask -- check-pr`
- `cargo run --locked -p xtask -- source-divergence`
- `git diff --check`

Expected after merge:

- `cargo run --locked -p xtask -- source-divergence` reports
  `new_source_commits=0`.
- Raw ancestry-only source/swarm divergence may remain nonzero under the
  repository's squash-merge policy.
