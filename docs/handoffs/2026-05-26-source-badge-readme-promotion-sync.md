# 2026-05-26 source badge and README promotion sync

Status: source-to-swarm promotion acknowledgement

This handoff records the source-side promotion PRs that brought the public
source repo back in line with swarm badge and README work. It is not new
analyzer work.

Source PRs:

| Source PR | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#492` | `cf7ea3e` | Release badge link points to `/releases/latest` | Already present from swarm `#408` |
| `EffortlessMetrics/unsafe-review#493` | `e75dfc2` | Main `unsafe-review` badge renders a numeric open-gap count | Already present from swarm `#409`; source endpoint count is source-specific |
| `EffortlessMetrics/unsafe-review#494` | `7f52d02` | `unsafe-review+` badge renders a numeric missing-or-weak evidence count | Already present from swarm `#410`; source endpoint count is source-specific |
| `EffortlessMetrics/unsafe-review#495` | `cf10348` | Facade crate README is the CLI entry point | Already present from swarm `#411` |
| `EffortlessMetrics/unsafe-review#496` | `5160e61` | Root README badge alt text matches numeric evidence badges | Partially present from swarm `#412`; this sync aligns the remaining main-badge alt text |

Swarm sync:

- `README.md` now describes the main endpoint badge as an open actionable gap
  count in alt text.
- `policy/source-sync.toml` acknowledges source main at
  `5160e61ec2abf8bca86ff066e316fd6856930856`.
- Swarm keeps its own generated badge endpoint counts because they are
  repo-scoped evidence projections.
- Swarm keeps its coverage badge row; source does not add that badge in this
  sync.

Validation:

- `cargo run --locked -p xtask -- check-docs`
- `cargo run --locked -p xtask -- check-pr`
- `cargo run --locked -p xtask -- source-divergence`
- `git diff --check`

Expected after merge:

- `cargo run --locked -p xtask -- source-divergence` reports
  `new_source_commits=0`.

Boundaries:

- no analyzer behavior change
- no coverage badge promotion to source
- no witness execution
- no automatic comments
- no source edits by `unsafe-review`
- no default blocking policy
- no safety, UB-free, Miri-clean, site-execution, precision, or recall claim
