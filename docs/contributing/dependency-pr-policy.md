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

## Choosing a mechanism for a new capability

The same risk instinct applies when *adding* a capability, not just bumping a
dependency. Pick the cleanest tool for the specific job, scoping cost to the
smallest population that works:

- A **shipped runtime dependency** taxes every user of the binary, forever — the
  highest-cost option; justify it against the alternatives.
- A **dev-dependency** lives only in the test build and ships nothing — much
  cheaper for a test or bench need.
- A **ledgered `unsafe` block** (`#[allow(unsafe_code, reason = …)]` plus the
  allow ledger, with a real `# Safety` contract) adds a bounded soundness
  obligation but **no shipped dependency** — for an in-product need with no good
  safe wrapper it is often *leaner* than a runtime crate. `unsafe` is forbidden
  by default but available as a governed exception when it is genuinely the
  cleanest tool.
- **External / CI / bench** measurement adds nothing to the binary — right for
  cadence or profiling concerns that do not belong on the per-PR path.

The rule is not "avoid `unsafe`" or "avoid dependencies": both are tools with
specific costs (a dependency = footprint + supply-chain + maintenance; `unsafe`
= a soundness obligation + ledger governance). Weigh those costs against the
alternatives *for the job at hand*, and do not spend a governed exception where
a free path suffices. See
[`ADR-0008`](../adr/UNSAFE-REVIEW-ADR-0008-resource-measurement-placement.md)
for a worked example: resource measurement is external-first (peak RAM on the
scheduled bench harness, no `unsafe` in the shipped binary), with in-product RSS
via a ledgered FFI implemented but parked pending validated demand — an
illustration of not spending a governed exception where a free path (external
measurement) suffices.

## Boundary

Dependency bumps do not change product claims or the advisory trust boundary.
This policy exists to avoid two specific failures: regressing behavior the core
gate does not cover, and parking-as-closing a still-useful bump.
