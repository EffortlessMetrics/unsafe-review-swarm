# 2026-07-19 - source dependency sync

Scope: acknowledge the two reviewed source dependency updates after their exact
dependency identity was mirrored into the swarm workbench.

This is a source-sync checkpoint. It does not publish crates, move a tag, cut a
release, execute witnesses, edit downstream source, or start policy gating.

## What source did

- Source PR #547 advanced `ignore` to 0.4.27 and merged as `fb217073`.
- Source PR #548 advanced `ra_ap_syntax` to 0.0.341 and merged as `4fc6eb80`.
- Source core, workspace, fixture, determinism, and repository gates passed for
  the directly affected dependency changes.

## What swarm absorbed

- Swarm PR #1958 mirrored both dependency versions and their lockfile graph.
- The issue #1916 machine-readable and human dependency-freeze records now
  carry the accepted source/swarm versions, commits, and lockfile hashes.
- Swarm-only implementation history remains in the workbench; this is a normal
  squash mirror, not a source-history import.

## Checkpoint

`policy/source-sync.toml` now acknowledges source main
`4fc6eb806de1460c618d60869b8a1cb885f87eea`. The expected
`source-divergence` result is `new_source_commits=0`.

## Evidence

- `cargo test -p unsafe-review-core --locked`: 880 tests passed on the rebased
  dependency sync branch.
- `cargo run --locked -p xtask -- check-pr`: passed for the dependency diff.
- Hosted `Unsafe Review Rust Result`, policy contracts, CodeRabbit,
  GitGuardian, and Graphite checks passed on swarm PR #1958.

## Trust boundary

This handoff records dependency and checkpoint identity only. It makes no
release-readiness, publication, safety, UB-free, Miri-clean, site-execution,
calibrated precision/recall, or policy-readiness claim.
