# 2026-05-21 - source promotion sync checkpoint

Scope: acknowledge `unsafe-review` source commit
`86112d1592d13a22371e42d06c46e8649cde6621`, the squash merge for
`unsafe-review#487`.

This is a checkpoint update, not a new implementation lane.

## Source commit

| Repository | PR | Commit | Meaning |
|---|---|---|---|
| `unsafe-review` | `#487` | `86112d1592d13a22371e42d06c46e8649cde6621` | Curated 0.2.0 public-usability promotion from the green swarm stack. |

## Why no base repair is needed

`#487` was a swarm-originated source promotion. Its source commit is new to
the `unsafe-review` history, but the promoted content came from
`unsafe-review-swarm` work that was already present in the workbench.

The promotion intentionally excluded live LSP runtime work:

- no `unsafe-review lsp` command,
- no `tower-lsp-server` or `tokio` dependency promotion,
- no VS Code extension,
- no witness execution,
- no automatic comments,
- no source edits,
- no default blocking policy.

## Checkpoint rule

`policy/source-sync.toml` now acknowledges source main at `86112d1` so routine
swarm work does not stop on a false source-ahead warning caused by the
promotion merge commit.

Future direct source commits still require one of:

- a swarm sync PR,
- a source-sync checkpoint update proving the source commit was a
  swarm-originated promotion whose content is already present in the workbench,
- or an explicit release/public-surface reason recorded in the source PR.

## Trust boundary

This checkpoint does not promote `unsafe-review` beyond its current advisory
posture:

- no memory-safety proof,
- no UB-free claim,
- no Miri-clean claim,
- no site-execution claim,
- no witness execution by default,
- no automatic comments,
- no source edits,
- no default blocking policy.

