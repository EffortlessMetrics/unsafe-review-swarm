# Changelog

This changelog starts with the post-0.3.2 unsafe-review workbench usability
lane. Earlier release targets and publication notes live in
[`docs/releases/`](docs/releases/).

`unsafe-review` remains advisory static review evidence. It does not prove UB,
memory safety, UB-free status, Miri-clean status, site execution, calibrated
precision/recall, or policy readiness, and it does not run witnesses, post
comments, edit source, or block by default.

## Unreleased

Nothing yet.

## 0.3.3 - 2026-06-05

0.3.3 is the Bun manual-candidate cockpit usability patch. It ships the
post-0.3.2 usability lane below. It remains advisory: manual candidates are
manually discovered (`analyzer_discovered = false`), confirmation cues are
emitted but never executed, and no proof, UB-free, Miri-clean, or
site-execution claim is made.

### Added

- Added per-card confirmation cues that frame each finding as a hypothesis
  pending external confirmation: `hypothesis_to_confirm`, `build_this_first`,
  `minimal_repro`, and `confirmation_step` are projected into `cards.json`,
  comment plans, agent context packets, and terminal `first-pr` output, and
  each cue states that unsafe-review did not run it.
  [#1431](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1431)
  [#1433](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1433)
  [#1435](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1435)
  [#1436](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1436)
  [#1456](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1456)
  [#1459](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1459)
- Added stable-byte manual-candidate metadata (`class`, `source`, `sink`,
  `hazard`, `observable`, `proof_required`, `suggested_fix_boundary`,
  `pr_aperture`, `ledger_state`) and surfaced it through `first-pr` and the
  GitHub summary while preserving the manual/advisory boundary.
  [#1422](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1422)
  [#1423](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1423)
- Added manual-candidate oracle maps (`rust_seam`, `oracle_language`,
  `oracle_path`, `oracle_kind`) with required node-parity oracle coverage for
  Bun-oriented candidates; oracle maps are routing context, not witness
  execution.
  [#1441](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1441)
  [#1497](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1497)
- Added ReviewCard proof-path projection across JSON, Markdown, comment-plan,
  witness-plan, and outcome outputs.
  [#1395](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1395)
- Added card evidence projection into `witness-plan.md`.
  [#1404](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1404)
- Added the `tokmd-packets.json` first-pr artifact: formatting-only manual
  packet inputs with comment budget, preset inputs, and manual repair item
  projection, recorded with `tokmd_run = false`.
  [#1412](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1412)
  [#1440](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1440)
  [#1450](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1450)
  [#1452](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1452)
- Added the manual repair handoff path: manual repair sidecar buckets, a
  review-kit manual-candidate mix summary, a repair-queue cockpit panel with
  agent-readiness cues, and repair-queue bucket reasons in summaries.
  [#1425](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1425)
  [#1427](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1427)
  [#1443](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1443)
  [#1449](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1449)
  [#1485](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1485)
- Added diff-scope file counts (`changed_files`, `changed_rust_files`,
  `changed_non_rust_files`) to summary JSON, reviewer summaries, and the
  review-kit manifest.
  [#1355](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1355)
  [#1356](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1356)
  [#1357](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1357)
- Added JSON and Markdown output formats for `repo --list-files` with recorded
  scan scope.
  [#1363](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1363)
- Added a dogfood drift guard requiring the Bun manual-candidate smoke report
  to list every committed manual-candidate example ID and primary file:line.
  [#1501](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1501)
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

- Unified the ReviewCard trust boundary across output surfaces and aligned
  public review claims: static unsafe contract review only; not memory-safety
  proof, not UB-free status, not Miri-clean status, and not a site-execution
  claim unless a matching witness receipt says so.
  [#1424](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1424)
  [#1491](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1491)
- Projected manual oracle and proof-mode context into the GitHub summary
  manual-candidate guidance.
  [#1490](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1490)
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

- Recorded stale-span-after-reentry detection and optional confirmation-cue
  execution as known next analyzer work, not implemented behavior.
  [#1503](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1503)
- Verified and documented the public UB-risk review workflow end to end.
  [#1476](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1476)
- Documented the evidence-machine repo style with CI and PR guidance.
  [#1461](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1461)
- Closed out the `get_unchecked` applicability burst with a maintained handoff
  covering pinned controls, false-positive rails, unclaimed limits,
  fixture-only versus dogfood-observed status, and fix-recipe mapping.
  [#1334](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1334)
- Promoted the usability docs and repair-queue readiness contract to the public
  source repository with history-preserving source catch-up.
  [source #520](https://github.com/EffortlessMetrics/unsafe-review/pull/520)
