# 0.4.0 release closeout — issue #1925

Status: `qualified_for_owner_decision` (not released, published, or public).
The repair lane is closed: every defect from the withdrawn verdict cleared
with proof (see machine-readable closeout).

Machine-readable source:
[`UNSAFE-REVIEW-CLOSEOUT-1925.toml`](../../plans/release-cutline/UNSAFE-REVIEW-CLOSEOUT-1925.toml)
(55 criteria generated 1:1 from the frozen cutline plus four release-gate rows).

## Candidate identity

| Field | Value |
| --- | --- |
| Version | `0.4.0` (core, CLI, facade; MSRV 1.98) |
| Source candidate | `a2b9cc48` (draft `unsafe-review#568`, unmerged) |
| Candidate tree | `e9b75262ab481d5dae7343014a80660f9a4a4625` |
| Swarm cutline / source base | `785e032d` / `c25d6527` (merge `fb7d7138`) |
| Candidate lockfile SHA-256 | `91f407bdfd16abc45d26c83ee640fc72963216836d7150bbd9888a6d6ec87d36` |
| Installed binary SHA-256 | `e56afd30ff8c45aa4773ef01606a649a74acb33c0d9c49dbc10bc5b628e9d73a` |
| Toolchain / platform | rustc 1.98.1; Linux x86_64 executed, Windows explicitly untested |
| As of | 2026-09-16 |

## Acceptance summary

51 cutline criteria plus 4 release-gate rows: **55 pass or deferred, 0
blocked required** (`ISSUE-2100` deferred per owner decision via PR #2213;
substrate merged, all gates green throughout — CI-diagnosability gap, not a
shipped-product defect). The installed receipt is rebound to final candidate
`a2b9cc48` via PR #2214 (byte-identical packaged inputs and executable).

Windows is executed: hosted run `35132286041` went 17/17 rows green on
`windows-latest` against the final `a2b9cc48` packages (rustc 1.98.1; receipt
bounds the evidence; installed LSP and pinned-consumer rows remain explicit
follow-up). The source gate is enforced: `unsafe-review/main` ruleset
`16511440` requires `Unsafe Review Rust Result` strict on the exact head with
merge commits only, and source PR `#568` verifies mergeable and clean at
`a2b9cc48`, left unmerged for the #1879 decision. Zero blocked required rows;
zero open environmental blockers.

Included: hosted rustdoc gate (#2203/#2209), active freeze (#1916/#2210),
frozen cutline (#1915/#2210), source candidate (#1917/#568), final docs
(#1918), installed qualification (#1921/#2211 with PR3 bounded rerun).
Deferred: prebuilt binaries, marketplace, Action `v1`, repair candidates,
grouping, robustness remainder, and the broader governance backlog — none
described as shipped. Rejected/abandoned: self-unsafe telemetry (#1620,
closed unmerged). Still open: swarm #2207/#2208 (deferred, keep-open),
source #515/#550/#551/#565 (supersede-via-candidate at merge).

## Included and deferred work

See the closeout TOML for the per-criterion record. No required implementation
remains local-only: scratch scripts live in `/tmp`, consumer clones are
read-only pins, and every committed change is on a pushed branch or merged PR.

## Known limits and trust boundary

Syntax-first analysis only; false-actionability residuals stand; partial,
capped, stale, and failed states are non-complete by contract; the editor loop
is read-only with no witness execution; Windows untested; Action `v1`,
prebuilts, marketplace, and crates.io availability are unavailable. This
closeout proves the frozen release-shape contract only — not safety, UB-free
status, accuracy, or authorization to publish.

## Owner publish handoff (copy-ready, NOT executed)

1. Re-fetch source PR `#568`; prove head `a2b9cc48` and checks unchanged.
2. Obtain explicit owner go — this closeout is not authorization.
3. Merge with a merge commit (never squash).
4. Publish core → CLI → facade; install from crates.io; rerun public smoke.
5. Tag `v0.4.0`, create the GitHub Release, record receipts, mirror to swarm.
6. Verify `source-divergence` shows no unacknowledged source commits.

## Rollback

Abandon: close `#568` unmerged with reason; keep the branch and receipts as
audit records. Nothing is published, so nothing needs retracting.
