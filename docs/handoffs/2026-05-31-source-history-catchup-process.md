# 2026-05-31 - source history catch-up process

Scope: document the repository-history repair process before preparing any
`0.3.1` crates.io patch.

This is process documentation. It does not publish crates, change versions, run
Bun, add analyzer behavior, execute witnesses, post comments, edit downstream
source, or start policy gating.

## Why this exists

The source repository can fall behind `unsafe-review-swarm` by enough reviewed
PR-squashed commits that source is missing product-relevant history and tree
state. When that happens, a narrow package hotfix is not enough. Source history
must be repaired first so the public source-of-record contains the reviewed
commit trail for the fix being published.

The immediate motivating case was a crates.io README image fix that existed in
swarm history but was not yet reachable from source history. Publishing a patch
directly from source before repairing history would have fixed one package
surface while leaving the deeper chain-of-custody problem unresolved.

## Documents added

- `docs/contributing/SOURCE_HISTORY_CATCHUP.md`
- `docs/releases/CRATES_IO_PATCH_RELEASE.md`

## Policy update

`docs/contributing/SWARM_TO_MAIN.md` now distinguishes:

- routine curated source promotion, which should stay narrow and recreate or
  cherry-pick exact behavior onto current source `main`, and
- exceptional source-history repair, which requires a real merge from
  `unsafe-review-swarm/main`, preservation of swarm ancestry, explicit conflict
  resolution, reachability checks, and merge-commit PR merge.

## Standing rule

Do not publish from a source repo that is missing the reviewed history for the
fix being published.

## Required next order

1. Finish the source history catch-up PR.
2. Merge it with a merge commit, not squash or rebase.
3. Verify key swarm commits are reachable from source `main`.
4. Then prepare any `0.3.1` crates.io package-surface patch.

## Trust boundary

This process does not change the product trust boundary. `unsafe-review`
remains advisory. It does not prove memory safety, UB-free status,
Miri-clean status, site execution, witness success, calibrated precision/recall,
or policy readiness. It does not execute witnesses, post comments, edit source,
or enforce blocking policy by default.
