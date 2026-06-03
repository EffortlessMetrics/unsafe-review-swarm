# 2026-06-03 source changelog history sync

Status: source-to-swarm history checkpoint sync

This handoff records source PR #521 and advances the swarm source-sync
checkpoint after source performed a history-preserving catch-up from swarm.
This is not a release, analyzer expansion, Bun finding, or policy-gate
promotion.

Source PRs and commits:

| Source PR / commit | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#521` | `c9fb2c90` | Preserved reviewed swarm changelog/usability history and source CI budget acknowledgement | Merged into swarm by this sync as source history and validation state |

Source PR #521 route:

- Source branch: `sync/usability-changelog`.
- Source merge commit:
  `c9fb2c905312759607ef27ec5380b25a4a4a46cd`.
- History-preserving catch-up merge:
  `03cb314c`.
- Source CI rerun acknowledgement commit:
  `b5f6cdd1`.

Swarm sync:

- `unsafe-review-swarm` absorbs the source main checkpoint from
  `c9fb2c905312759607ef27ec5380b25a4a4a46cd`.
- `policy/source-sync.toml` acknowledges source main at
  `c9fb2c905312759607ef27ec5380b25a4a4a46cd`.
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
