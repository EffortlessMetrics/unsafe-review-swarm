# 2026-06-01 source badge plus count sync

Status: source-to-swarm public badge projection sync

This handoff records `EffortlessMetrics/unsafe-review#512`, which changed the
public `unsafe-review+` Shields endpoint to report only missing-or-weak
evidence-quality findings instead of adding the open actionable gap count again.

Source PRs and commits:

| Source PR / commit | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#512` | `bee8e234` | Public badge endpoint semantics and docs | Mirrored by this sync |

Swarm sync:

- `crates/unsafe-review-core/src/output/badges.rs` mirrors the source
  `unsafe-review+` count formula.
- `badges/unsafe-review-plus.json` mirrors the regenerated Shields endpoint
  payload.
- `README.md`, `docs/BADGE_POLICY.md`, and badge-related specs mirror the
  source wording that keeps badge meaning in docs rather than endpoint message
  text.
- `policy/source-sync.toml` acknowledges source main at
  `bee8e234d879316da3215d9ec2a42a5b02a8fbc2`.

Boundaries:

- no analyzer behavior change
- no calibration expansion
- no publication claim
- no Bun vulnerability claim
- no witness execution
- no automatic comments
- no source edits by `unsafe-review`
- no default blocking policy
- no safety, UB-free, Miri-clean, site-execution, precision, recall, or
  policy-readiness claim

Validation:

- `cargo fmt --check`
- `cargo test -p unsafe-review-core badge --locked`
- `cargo test -p unsafe-review --test e2e repo_inventory_and_badges_count_open_gaps_without_safety_claim --locked`
- `cargo test -p xtask public_badge --locked`
- `cargo run --locked -p unsafe-review -- badges --out badges/`
- `cargo run --locked -p xtask -- check-docs`
- `cargo run --locked -p xtask -- check-pr`
- `cargo run --locked -p xtask -- source-divergence`
- `git diff --check`

Expected after merge:

- `cargo run --locked -p xtask -- source-divergence` reports
  `new_source_commits=0`.
