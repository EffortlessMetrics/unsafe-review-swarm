# 2026-06-03 source manual candidate handoff cross-check sync

Status: source-to-swarm history checkpoint sync

This handoff records source PR #522 and advances the swarm source-sync
checkpoint after source performed a history-preserving catch-up from swarm.
This is not a release, analyzer expansion, Bun finding, or policy-gate
promotion.

Source PRs and commits:

| Source PR / commit | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#522` | `7d87fa78` | Preserved reviewed swarm manual-candidate first-pr handoff cross-checks and source CI budget acknowledgement | Merged into swarm by this sync as source history and validation state |

Source PR #522 route:

- Source branch: `sync/manual-candidate-handoff-crosscheck`.
- Source merge commit:
  `7d87fa782e8c9fd333d2c2436ec917207f0cd5c2`.
- History-preserving catch-up merge:
  `b51ac178512d0ab2d8e4f456dcbff39554b001b4`.
- Source CI rerun acknowledgement commit:
  `0de07a14500f714f77b2d1e480da74412ad9b64c`.
- Swarm feature commit preserved by source:
  `8b9582b9`.

Swarm sync:

- `unsafe-review-swarm` absorbs the source main checkpoint from
  `7d87fa782e8c9fd333d2c2436ec917207f0cd5c2`.
- `policy/source-sync.toml` acknowledges source main at
  `7d87fa782e8c9fd333d2c2436ec917207f0cd5c2`.
- Existing swarm-only workbench commits remain unpromoted until deliberately
  promoted to source.
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
