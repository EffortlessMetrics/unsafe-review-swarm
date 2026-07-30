# 2026-07-29 - source policy sync

Scope: acknowledge the two source policy/documentation commits that followed
the dependency checkpoint. This is a source-sync checkpoint, not a release or
publication operation.

## What source did

- Source PR #558 removed residual RTK command routing from `AGENTS.md`,
  `CLAUDE.md`, and the documentation-automation policy, merged as
  `768d6ccc343fe23310ca544436152f3910dcd77a3`.
- Source PR #559 removed residual RTK command guidance from the source
  documentation set, merged as
  `c25d65272c760c3630eb9528b7efaae2234d9e19`.

## What swarm already carries

The effective tree changes are already represented in the workbench by merged
PRs #1971, #1973, and #1975. Cherry-picking the two source policy commits on
current swarm `main` produced no additional tree changes after preserving the
newer swarm source-of-truth, dogfood, and objective-audit wording. The two
source dependency commits in the preceding checkpoint were likewise already
represented by the acknowledged dependency mirror.

## Checkpoint

`policy/source-sync.toml` now acknowledges source main at
`c25d65272c760c3630eb9528b7efaae2234d9e19`. The expected
`source-divergence` result is `new_source_commits=0`.

## Trust boundary

This handoff records source ancestry and documentation/policy parity only. It
does not publish crates, move a tag, execute witnesses, edit downstream source,
or claim safety, proof, UB-free status, Miri cleanliness, site execution,
calibrated precision/recall, or policy readiness.
