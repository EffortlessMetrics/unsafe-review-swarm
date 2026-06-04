# Dogfood report: 2026-06-03 Bun manual candidates first-pr smoke

Status: focused manual-candidate projection smoke report
Swarm commit: `37678918` initial smoke; later manual-candidate example slices
extend the same checked example set.
Artifact status: local, untracked under `target/unsafe-review-manual-candidate-smoke/`

This report records the committed Bun manual-candidate examples as a fixture
control for `first-pr` manual-candidate projection. The goal is to keep
externally discovered Bun findings useful as copy-only reviewer and implementer
handoffs without treating them as analyzer-discovered ReviewCards.

It is not a Bun runtime run, support-tier promotion, calibration report, policy
decision, safety proof, UB-free claim, Miri-clean claim, witness result, or
site-execution proof. No witness tools were run.

## Scope

Target:

- `bun-manual-candidates-first-pr-smoke`

Command:

```bash
rtk cargo run --locked -p xtask -- check-manual-candidate-examples
```

The command imports all committed examples under
`docs/examples/manual-candidates/` into a disposable first-pr fixture, writes a
full advisory bundle under `target/unsafe-review-manual-candidate-smoke/`, and
runs the first-pr artifact verifier.

## Summary

| Surface | Result | Reviewer note |
|---|---:|---|
| `manual-candidates.json` | Verified | Preserves sorted manual IDs, manual/advisory markers, source route, invariant, evidence commands and limitations, fix options, test targets, do-not-touch notes, and trust boundary from the committed examples. |
| `manual-repair-queue.json` | Verified | Preserves the same manual IDs, implementer handoff, fix/test/non-goal guidance, and copy-only commands as `manual-candidates.json`; it is not the ReviewCard repair queue and does not run an agent. |
| `review-kit.json` | Verified | Includes a bounded manual-candidate queue with implementer handoff cues that match `manual-candidates.json`; candidates stay out of the ReviewCard repair queue. |
| `pr-summary.md` | Verified | Shows a compact manual-candidate front door with count, first candidate, queue preview, commands, guidance, and advisory boundary. |
| `github-summary.md` | Verified | Keeps the doorway bounded and points to the manual-candidate index instead of duplicating full packets. |
| `witness-plan.md` | Verified | Adds manual-candidate follow-up cues while keeping them outside ReviewCard witness route groups. |
| `repair-queue.json` | Verified | Remains ReviewCard-only and free of manual-candidate markers. |

## Bun candidates

- `R4R2-S001`: TextDecoder SharedArrayBuffer route at
  `src/runtime/webcore/TextDecoder.rs:237`.
- `R4R2-S002`: MySQL BLOB SharedArrayBuffer bind route at
  `src/sql_jsc/mysql/MySQLValue.rs:411`.
- `R4R2-S003`: zlib/Zstd overlapping-buffer contract route at
  `src/runtime/node/node_zlib_binding.rs:207`.
- `R4R2-S004`: async StringOrBuffer resizable-ArrayBuffer stale-input route at
  `src/runtime/node/types.rs:402`.
- `R4R2-S005`: node:fs async scalar write resizable-ArrayBuffer stale-input
  route at `src/runtime/node/node_fs.rs:3795`.

The useful cockpit behavior is that all entries remain manual candidates,
carry file:line and safe-caller route context, preserve external evidence
commands and limitations, and expose copy-only fix/test/non-goal guidance for a
future Bun implementer lane.

## Triage observation

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `bun-manual-candidates-first-pr-smoke` | manual-candidate projection bundle | `actionable` | The committed Bun examples verify across manual-candidates JSON, manual repair queue, review-kit handoff, PR-summary, GitHub-summary, witness-plan, and ReviewCard repair-queue boundaries while preserving manual/advisory markers. | Re-run this smoke after future manual-candidate import, first-pr, review-kit, manual repair queue, witness-plan, implementer-handoff projection, or Bun example-packet changes. |

## Trust boundary

This is static unsafe contract review dogfood for manual-candidate projection.
It does not prove memory safety, UB-free status, Miri-clean status,
unsafe-site execution, witness adequacy, release readiness, or policy
readiness. It is not calibrated precision or calibrated recall evidence.
`unsafe-review` did not run witnesses, post comments, edit source, run an
agent, or enforce blocking policy.
