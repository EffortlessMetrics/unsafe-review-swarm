# 2026-06-03 source usability docs and repair readiness sync

Status: source-to-swarm history and validation sync

This handoff records source PR #520 and advances the swarm source-sync
checkpoint after source performed a history-preserving catch-up from swarm.
This is not a release, analyzer expansion, Bun finding, or policy-gate
promotion.

Source PRs and commits:

| Source PR / commit | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#520` | `09785cbb` | Preserved reviewed swarm history through `560adc8f`, promoted usability docs and repair queue readiness, and kept source validation green with follow-up test hygiene | Merged into swarm by this sync as source history and validation state |

Source PR #520 route:

- History-preserving source catch-up from previous source parent
  `7c34690d9944a0d0b9f403a3dddcb02a2b3e9f15`.
- Swarm parent imported by source:
  `560adc8fe3adc179ac6a6acb6fb5d8da8d6a5b4d`.
- Catch-up merge commit:
  `2abba0be63b14e6835979f140bc7b66da3b3d2f0`.
- Source validation fixes:
  `b1904242` and `dff6d8c2`.
- CI rerun acknowledgement commit:
  `55f52ee8`.

Swarm sync:

- `unsafe-review-swarm` absorbs the source main tree state from
  `09785cbb344912ad14a102041239d90a434177ad`.
- `policy/source-sync.toml` acknowledges source main at
  `09785cbb344912ad14a102041239d90a434177ad`.
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

- `cargo test -p unsafe-review manual_candidate_list_reports_imported_advisory_ledger -- --nocapture`
- `cargo run --locked -p xtask -- check-docs`
- `cargo run --locked -p xtask -- check-pr`
- `cargo run --locked -p xtask -- source-divergence`
- `git diff --check`

Expected after merge:

- `cargo run --locked -p xtask -- source-divergence` reports
  `new_source_commits=0`.
- Raw ancestry-only source/swarm divergence may remain nonzero under the
  repository's squash-merge policy.
