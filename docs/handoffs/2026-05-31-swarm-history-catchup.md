# 2026-05-31 - swarm history catch-up

Scope: preserve reviewed `unsafe-review-swarm` history in the public
`unsafe-review` repository after the `0.3.0` advisory review cockpit release.

## Reason

`unsafe-review-swarm` is the development workbench and already squash-merges
one reviewed PR into one `main` commit. `unsafe-review` should not flatten those
reviewed commits into another source sync commit.

This catch-up imports `unsafe-review-swarm/main` with a real merge so source can
retain the swarm commit graph, including product-relevant packaging and review
cockpit commits.

## Required merge mode

The source PR containing this handoff must be merged with a merge commit. Do not
squash or rebase it. Squashing would drop the swarm parent and defeat the
history-preservation goal.

## Boundary

This is source history and release-closure repair. It is not a Bun lane and does
not claim proof, UB-free status, Miri-clean status, site execution, calibrated
precision or recall, policy readiness, witness execution, automatic comments,
source edits, or default blocking policy.
