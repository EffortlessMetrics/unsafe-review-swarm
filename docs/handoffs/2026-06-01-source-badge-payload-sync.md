# 2026-06-01 source badge payload sync

Status: source-to-swarm public badge surface sync

This handoff records the source-only badge payload hardening from
`EffortlessMetrics/unsafe-review#510` and mirrors it into the swarm workbench so
routine swarm work is not based on a stale public surface.

Source PRs and commits:

| Source PR / commit | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#510` | `06ba8ae4` | Emits shields-safe public badge payloads and updates badge policy/tests | Mirrored by this sync as a source public-surface fix |

Swarm sync:

- `README.md` badge text/links follow the source badge payload wording.
- `badges/unsafe-review.json` and `badges/unsafe-review-plus.json` mirror the
  source public badge payload shape.
- `crates/unsafe-review-core/src/output/badges.rs` emits shields-safe schema,
  label, message, color, and named logo fields.
- `crates/unsafe-review/tests/e2e.rs` checks the public badge payloads.
- `docs/BADGE_POLICY.md` documents the public badge payload constraints.
- `xtask/src/public_badges.rs` mirrors the source public badge validator.
- `policy/source-sync.toml` acknowledges source main at
  `06ba8ae4f3b2aa17b21a3cff067e0bc8543151a5`.

Boundaries:

- no crates.io publication claim
- no tag or GitHub Release claim
- no witness execution
- no automatic comments
- no source edits by `unsafe-review`
- no default blocking policy
- no safety, UB-free, Miri-clean, site-execution, precision, recall, or
  policy-readiness claim

Validation:

- `cargo fmt --check`
- `cargo test --locked -p unsafe-review badge`
- `cargo run --locked -p xtask -- check-public-surfaces`
- `cargo run --locked -p xtask -- check-pr`
- `cargo run --locked -p xtask -- source-divergence`
- `git diff --check`

Expected after merge:

- `cargo run --locked -p xtask -- source-divergence` reports
  `new_source_commits=0`.
