# Changelog

This changelog starts with the post-0.3.2 unsafe-review workbench usability
lane. Earlier release targets and publication notes live in
[`docs/releases/`](docs/releases/).

`unsafe-review` remains advisory static review evidence. It does not prove UB,
memory safety, UB-free status, Miri-clean status, site execution, calibrated
precision/recall, or policy readiness, and it does not run witnesses, post
comments, edit source, or block by default.

## Unreleased

### Added

- Added the public find/fix workflow for UB-risk review seams:
  `doctor`, `first-pr`, `pr-summary.md`, `explain`, `context --json`,
  `witness-plan.md`, receipt audit, and outcome comparison now have a single
  maintainer path. See [docs/FIND_AND_FIX_UB.md](docs/FIND_AND_FIX_UB.md).
  [#1337](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1337)
- Added ReviewCard fix recipes by operation family for `get_unchecked`,
  `MaybeUninit::assume_init*`, `Vec::set_len`, UTF-8 unchecked conversion,
  pointer copies, `NonNull`, raw pointer reads/writes, transmute, FFI/unsafe
  calls, and target-feature/inline-asm review. The recipes describe what
  evidence matters, good and bad repairs, witness routes, and what the recipe
  does not prove.
  [#1340](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1340)
- Added the bounded agent repair workflow for `repair-queue.json` and
  `context <card-id> --json`, including allowed repairs, do-not-do rules, stop
  conditions, receipt handling, and reviewer responsibility.
  [#1342](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1342)
- Added the advisory UB-risk CI cookbook: run `first-pr`, upload the review kit,
  append `github-summary.md`, optionally emit SARIF, and avoid automatic
  comments or blocking by default.
  [#1343](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1343)
- Added dogfood usefulness judgment records so real review-kit cards can be
  labeled `actionable`, `noise`, `missed`, `uncertain`, `good-agent-task`, or
  `bad-agent-task` without implying calibrated precision/recall.
  [#1324](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1324)

### Changed

- Made repair-queue agent readiness a closed contract:
  `ready_for_agent`, `requires_human_review`, `requires_witness_receipt`, and
  `unsupported`. The verifier now enforces that `ready = true` means
  `ready_for_agent`, and `ready = false` means any non-agent-ready state.
  [#1332](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1332)
- Added PR disposition policy: out-of-lane aligned work should be deferred,
  drafted, or blocked rather than closed; close only duplicate, superseded,
  rejected, abandoned, or unrecoverable work.
  [#1329](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1329)
- Projected manual candidate evidence and implementer handoff details into the
  candidate list path while preserving the manual/advisory boundary.
  [#1345](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1345)

### Documented

- Closed out the `get_unchecked` applicability burst with a maintained handoff
  covering pinned controls, false-positive rails, unclaimed limits,
  fixture-only versus dogfood-observed status, and fix-recipe mapping.
  [#1334](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1334)
- Promoted the usability docs and repair-queue readiness contract to the public
  source repository with history-preserving source catch-up.
  [source #520](https://github.com/EffortlessMetrics/unsafe-review/pull/520)
