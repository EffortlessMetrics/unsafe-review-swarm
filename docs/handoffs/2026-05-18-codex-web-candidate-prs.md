# Codex web candidate PR intake

Date: 2026-05-18
Status: candidate queue drained; no open Codex-web candidate PRs remain
Base snapshot: `main` at `a9274c2` after `docs: define dogfood-calibrated evidence lane`

## Operating model

The main agent lane remains sequential and merge-ready. Codex web PRs are
candidate branches, not failures and not active-lane blockers.

Current PR close/defer decisions must follow
[`SWARM_TO_MAIN.md`](../contributing/SWARM_TO_MAIN.md#pr-disposition-policy).
This handoff remains the historical candidate queue ledger.

Use this intake rule for every candidate:

- Does it improve the current active lane?
- Does it preserve the `ReviewCard` contract?
- Does it avoid new policy authority or default blocking?
- Does it avoid creating another source of truth?
- Is it narrow enough to review?
- Can it be rebased cleanly onto current `main`?

Disposition choices:

- `merge`: review, rebase if needed, validate, and merge now.
- `rework`: useful idea, but split or adjust before merge.
- `park`: useful later-lane branch; leave open or convert to issue.
- `close-duplicate`: close only after a canonical PR for the theme is chosen.

## Current lane context

The original intake snapshot covered PR/CI projection from existing review
cards. The repo has since closed PR/CI projection, saved LSP/agent projection,
repo policy, first witness receipt import slices, fixture calibration, real-crate
dogfood, and the first saved-snapshot outcome comparison. The active lane is now
defined in `docs/status/DOGFOOD_CALIBRATED_EVIDENCE_LANE.md`.

This document remains the candidate inventory ledger and closeout record. It
does not authorize merging stale candidate branches as-is. Useful slices should
be rebuilt narrowly on current `main` when their target lane opens.

Already landed in this lane:

- PR summary artifact
- SARIF artifact
- advisory GitHub workflow
- inline comment planning artifact
- scanner false-positive hardening that protects PR/LSP/agent projections
- fixture validation, focused unit coverage, and CLI artifact e2e coverage
- advisory artifact verifier and projection consistency checks
- raw pointer write, unaligned read, and partial-parse scanner hardening
- CLI diff-input hardening for stdin diffs, root-relative diffs, and local `--out`
- repo-mode dogfood false-positive hardening for deref assignments in product code
- CLI parser hardening for `--flag=value` artifact commands and missing values
- core property-test hardening for diff-coordinate and identity-token invariants
- mutation-sensitive test extraction for obligation mappings and scanner scope behavior
- classifier route-precedence coverage for concurrency and FFI cards
- manual cargo-fuzz analyzer harness without scheduled or blocking workflow
- raw pointer write method-form scanner coverage
- review-card trust-boundary explanation docs
- optional calibration expectation type validation
- cargo subcommand wrapper e2e coverage
- subcommand-position help flag handling
- support-summary posture validation and operation-registry obligation-key
  validation
- diff parser tests for multihunk diffs, new-file additions, and the exact
  six-line review window used by changed-line matching
- obligation-family tests for raw pointer deref/read/write, unaligned raw
  pointer operations, FFI, and unknown operation review models
- cargo-subcommand wrapper coverage for PR summary artifact writing
- classifier severity tuple coverage for ordinary evidence states
- safe-repo CLI no-noise coverage
- syntax line/column property coverage
- manual fuzz harness shape validation wired into `check-pr` without adding a
  fuzz workflow
- Dependabot Cargo and GitHub Actions update inbox without workflow behavior,
  policy authority, or default blocking changes
- same-receiver `if let ... as_ref()` evidence for `unwrap_unchecked`, with a
  false-positive-control fixture proving another receiver does not discharge the
  valid-value obligation

Do not use candidate intake as permission to jump to release work, default
blocking, automatic comments, witness execution by default, broad workflow
surface, or broad refactors. Receipt audit and outcome movement work belongs in
explicit dogfood-calibration PRs, not stale candidate branches.

## Candidate labels

Use these labels on open candidate PRs:

- `candidate`
- `candidate:tests`
- `candidate:docs`
- `candidate:ci`
- `candidate:fuzz`
- `candidate:mutation`
- `candidate:refactor`
- `candidate:scanner`
- `candidate:pr-projection`

These labels were used for candidate intake. As of the latest closeout, the
open candidate queue is empty.

## Theme inventory

| Theme | Candidate PRs | Current canonical | Disposition | Target lane | Reason |
|---|---:|---|---|---|---|
| Scanner false-positive hardening | #29, #32 | merged as #100 | closed | current hardening | PR #27 merged the broad class; #100 extracted the remaining syntax-declaration false-positive guard without reopening broad scanner changes. |
| Scanner partial-parse recovery | #48 | merged as #103 | closed | current hardening | #103 keeps syntax-backed concrete operation detection available when unrelated parse errors exist, reducing fake unknown wrapper cards in PR artifacts. |
| Raw pointer write detection | #49, #62 | merged as #95 | closed | current hardening | #95 kept syntax-target behavior and fixture proof, rebuilt narrowly on current main as raw pointer assignment-write detection. |
| xtask fixture validation | #33, #50, #63, #64 | merged as #92 | closed | current hardening | Fixture validation protects the support-tier proof mechanism by validating fixture layout, golden JSON shape, diff shape, and package naming without broad xtask policy changes. |
| CLI e2e coverage | #39, #58, #80, #81 | merged as #94, #289, #350, and #353 | closed | current PR/CI projection | #94 kept #81's user-path shape but updated it for landed PR artifacts: JSON, PR summary, SARIF, comment plan, context, and explain. #289 extracted the remaining useful cargo-subcommand wrapper path. #350 added cargo-subcommand wrapper coverage for PR summary artifact writing, and #353 added no-noise safe repo output coverage. |
| Focused unit coverage | #44, #61, #86, #87 | merged as #93, #284, #349, #352, and #376 | closed | current hardening | #93 is the canonical core-only slice for classifier, evidence, and diff parser invariants. #284 added the remaining route-precedence regression coverage for concurrency and FFI cards. #349 added multihunk and new-file diff parser coverage, #352 pinned ordinary evidence-state class, priority, and confidence tuples, and #376 extracted the remaining useful obligation-family assertions for raw pointer, unaligned raw pointer, FFI, and unknown operation models. |
| Property testing | #43, #60, #84, #85 | merged as #115 and #354 | closed | later hardening | #115 extracted the narrow core invariant slice: unified-diff new-file coordinates, removed-only file tracking, slug token stability, and path-display normalization. #354 added syntax line/column coordinate property coverage. These added no fuzz workflow, mutation workflow, product surface, or policy authority. |
| Fuzzing | #42, #59, #82, #83 | merged as #285 and #355 | closed | later hardening | #285 extracted only the useful manual analyzer fuzz harness. #355 validates the harness shape and docs in `check-pr` without compiling fuzz or adding a workflow. Scheduled or blocking fuzz workflow surface remains out of scope. |
| Mutation-sensitive tests | #41, #79 | merged as #119, #351, and #352 | closed | current hardening | #119 extracted the current-lane-safe tests for obligation/hazard mapping, concrete-operation suppression, and diff-vs-repo scanner filtering. #351 pinned the exact changed-line review window boundary, and #352 pinned classifier severity tuples. #41 and #79 were closed as superseded. |
| Mutation workflow/config | #57, #78 | merged as #286 for test-only slice | closed | later hardening | #286 extracted raw pointer write method-form scanner tests from the stale mutation candidates. Mutation workflow/config surface remains deferred. |
| CLI ergonomics and diff handling | #31, #45, #46, #47 | merged as #104, #112, and #290 | closed | current PR/CI projection | #104 kept current-lane fixes: stdin diffs, root-relative diff files, current-directory `--out`, JSON aliases, duplicate card-id rejection, and no `--fail-on-gaps` policy behavior. #112 extracted the useful current-lane slice from stale #46: `--flag=value` parsing, stricter missing-value handling, and help text aligned with advisory artifact outputs. #290 extracted subcommand-position `--help` / `-h` handling. |
| Documentation usage guides | #36, #37, #53, #56, #72, #73, #76, #77 | merged as #143, #144, and #287 | closed | current docs | #143 added the canonical current-surface CLI guide. #144 extracted the crate README link. #287 extracted review-card trust-boundary explanation docs. Direct duplicates were closed as superseded. |
| Diataxis docs structure | #35, #54, #70, #71 | none | closed | later docs | Broad documentation restructuring was closed as stale option inventory; future docs work should be rebuilt from current product surfaces. |
| Spec expansion | #38, #52, #68, #69 | none | closed | later source-of-truth | Broad spec drafts were closed as stale option inventory. Specs should follow concrete behavior gaps and proof artifacts, not outrun implementation. |
| CI hardening | #34, #51, #65, #66 | merged as #96 and #363 | closed | current hardening | #96 kept the narrow workflow reliability pieces: read-only permissions, no persisted checkout credentials, locked Cargo commands, docs build, timeout, manual dispatch, and PR-run cancellation. #363 extracted only the Dependabot update inbox for Cargo and GitHub Actions maintenance signals, without adding blocking policy, witness execution, or automatic comments. |
| Broad module refactors | #40, #55, #74, #75 | none | closed | later refactor | Broad SRP churn was closed as stale option inventory. Rebuild only if it directly unlocks a reviewed implementation slice. |
| Public JSON/visibility API | #28 | merged as #101 | closed | current hardening | `UnsafeSite` already tracked visibility and public API surface; #101 projected those fields into JSON and updated fixture goldens. |
| Unaligned raw pointer read behavior | #30 | merged as #102 | closed | current hardening | #102 kept the useful distinction that `read_unaligned` does not require alignment evidence while preserving other raw pointer read obligations. |

## Open candidate disposition

No remaining Codex-web candidate PR is open or an active merge candidate for the
dogfood-calibrated evidence lane. The remaining ideas are option inventory only:

| Former PRs | Theme | Disposition | Target lane | Reason |
|---:|---|---|---|---|
| #42, #59, #82, #83 | Fuzzing | extracted and closed | later hardening | #285 kept the compileable manual harness. #355 added shape validation to `check-pr`; scheduled or blocking fuzz workflow surface remains deferred. |
| #57, #78 | Mutation workflow/config | extracted and closed | later hardening | #286 kept scanner regression tests. #351 and #352 kept useful mutation-sensitive diff/classifier assertions. Mutation workflows remain deferred. |
| #76 | Documentation overview | superseded and closed | later docs | #287 kept the useful trust-boundary explanation slice. Broad overview docs should be rebuilt from current surfaces if needed. |
| #35, #54, #70, #71 | Diataxis docs structure | closed | later docs | Broad documentation restructuring is not active-lane work and risks creating competing source-of-truth pages. |
| #38, #52, #68, #69 | Spec expansion | closed | later source-of-truth | Specs should follow concrete behavior gaps and proof artifacts, not outrun implementation. |
| #40, #55, #74, #75 | Broad module refactors | closed | later refactor | Avoid scanner/module churn unless it directly unlocks a reviewed implementation slice in the active lane. |
| #34, #51, #65, #66 | CI maintenance extras | extracted and closed | current hardening | #363 kept the Dependabot inbox slice. Broader cache/workflow expansion remains deferred unless it directly protects a current evidence-loop artifact. |

If any parked candidate becomes useful, choose one canonical PR for that theme,
rebase or rebuild it on current `main`, and land only the narrow slice that maps
to the active lane.

## Immediate intake order

1. Candidate intake for review cards, PR artifacts, projections, repo posture,
   receipt foundation, fixture calibration, first dogfood slices, fuzz harness,
   CLI wrapper/help behavior, and trust-boundary docs is drained.
2. No remaining open candidate branch is an active merge candidate.
3. Do not merge parked branches as-is.
4. Rebuild useful slices narrowly on current `main` when they directly support
   the dogfood-calibrated evidence loop.
5. Close duplicate or superseded PRs only after a canonical replacement or
   durable issue captures the useful idea.

## Review protocol

For each canonical candidate:

```bash
rtk git fetch origin
rtk git switch <candidate-branch>
rtk git rebase origin/main
rtk cargo fmt --check
rtk cargo check --workspace --all-targets
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test --workspace
rtk cargo xtask check-pr
```

If the branch is valuable but too broad, rebuild the useful slice on a fresh
branch instead of merging it as-is.
