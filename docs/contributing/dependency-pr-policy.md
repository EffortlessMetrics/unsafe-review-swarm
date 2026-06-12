# Contributing: dependency PR policy

Dependency bumps (Dependabot or manual) are **risk-routed**, not blanket-merged
and not blanket-deferred. The required core gate (`check-pr` plus the workspace
tests) must pass for any bump; beyond that, route by what the dependency
actually touches.

## Risk routing

| Dependency kind | Examples | Required before merge |
|---|---|---|
| Parser / syntax | `ra_ap_syntax` | Analyzer/parser tests green — the source-text analyzer depends on parsing fidelity, so a parser change can silently shift detection. |
| Signal / process control | `signal-hook` | Cancellation / timeout / SIGINT tests — the scan's interrupt and partial-status behavior depends on signal handling. |
| Repo traversal | `ignore`, `globset`, `walkdir` | Discovery / nested-checkout / large-repo scoping tests — traversal changes can alter which files are scanned or skipped. |
| CI actions | `github-actions` group | Pinning + permissions + workflow-behavior review — actions are a supply-chain and permissions surface, not just a version bump (and adding/altering a workflow needs its `policy/workflow-allowlist.toml` ledger entry). |
| Other patch/minor | most crates | Core gate green is sufficient. |

## Major bumps

A major version bump (for example `signal-hook 0.3 -> 0.4`) requires a
**targeted test for the changed surface** before merge. Do not merge a major
blind on a green core gate — the core gate may not exercise the changed
behavior, and the bump is exactly where that behavior moves.

## Disposition

A dependency PR that is correct but out of the current lane is **deferred /
parked, not closed** (see PR-queue discipline in `AGENTS.md`). Record what test
or review it still needs so a later pass can pick it up without re-deriving the
risk.

Currently parked (as of 2026-06-11):

- `#1565` (github-actions group, 3 updates) — needs pinning/permissions review
  and the `policy/workflow-allowlist.toml` ledger entry re-applied after rebase.
- `#1390` (`signal-hook` 0.3.18 -> 0.4.4, major) — needs a targeted
  SIGINT / timeout / partial-status test for the new API before merge.

## Boundary

Dependency bumps do not change product claims or the advisory trust boundary.
This policy exists to avoid two specific failures: regressing behavior the core
gate does not cover, and parking-as-closing a still-useful bump.
