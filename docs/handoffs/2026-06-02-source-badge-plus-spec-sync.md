# 2026-06-02 source badge plus spec sync

Status: source-to-swarm public badge surface sync

This handoff records the source-only badge specification wording correction from
`EffortlessMetrics/unsafe-review#517` and mirrors it into the swarm workbench so
routine swarm work is not based on stale `unsafe-review+` badge semantics.

Source PRs and commits:

| Source PR / commit | Source commit | Surface | Swarm status |
|---|---|---|---|
| `EffortlessMetrics/unsafe-review#517` | `7c34690d` | Aligns the `unsafe-review+` acceptance example with the evidence-quality component sum and removes stale open-gap addition wording | Mirrored by this sync as a source public-surface docs fix |

Swarm sync:

- `docs/specs/UNSAFE-REVIEW-SPEC-0014-repo-inventory-badges.md` mirrors the
  source acceptance wording for `unsafe-review+`.
- `policy/source-sync.toml` acknowledges source main at
  `7c34690d9944a0d0b9f403a3dddcb02a2b3e9f15`.

Boundaries:

- no crates.io publication claim
- no tag or GitHub Release claim
- no analyzer behavior change
- no badge count import from source
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
