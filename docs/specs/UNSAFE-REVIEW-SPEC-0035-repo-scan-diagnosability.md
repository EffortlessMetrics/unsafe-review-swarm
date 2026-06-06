# UNSAFE-REVIEW-SPEC-0035: Repo-scan diagnosability

Status: proposed
Owner: product / cli
Created: 2026-06-06
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- plans/0.2.0/implementation-plan.md
Linked issues:
- none
Linked PRs:
- TBD
Support-tier impact: repo-mode scan posture
Policy impact:
- none

## Problem

A PR gate on a large repository depends on the repo-mode scan
(`unsafe-review repo`) completing, or failing loudly. The behavior already
exists and passed release smoke — include/exclude filters, large-repo default
ignores, `--list-files`, `--progress` heartbeats, `--timeout-seconds` with a
status sidecar, write-then-rename partial output, SIGTERM/SIGINT diagnostics,
and `file:line` in markdown. What is missing is a **stable contract**: nothing
pins the discovery filters, the status-sidecar schema, or the
no-silent-failure guarantee, so a refactor could regress them and a consumer
has no documented surface to depend on.

`unsafe-review` is the coverage instrument (UNSAFE-REVIEW-SPEC-0028); the
repo-mode scan is the projection that produces the per-card coverage block
defined in UNSAFE-REVIEW-SPEC-0029 across a whole tree. This spec documents the
contract and adds verification. It introduces no analyzer behavior and builds
nothing new. "Easy" here means a large-repo coverage scan **never fails
silently and is always diagnosable**.

## Behavior

### Discovery filters (stable surface)

Input scope (UNSAFE-REVIEW-SPEC-0003) is selected by a fixed set of controls:

```text
--include <glob>        additive include globs (repeatable)
--exclude <glob>        subtractive exclude globs (repeatable)
default ignores         target/, .git/, vendored/generated trees ignored by default
--respect-gitignore     honor .gitignore (default on)
--max-files <n>         hard upper bound on discovered files; refuse, do not truncate silently
```

Exclude wins over include; default ignores and `--max-files` keep an unbounded
tree bounded. Crossing `--max-files` is reported as a diagnostic with the
operator next step, not a silent partial.

### list-files / dry-run is bounded and honest

`unsafe-review repo --list-files` enumerates exactly the files the scan would
read, under the same filters, and exits without analyzing. The list is bounded
by the same `--max-files` ceiling and states the discovery scope it used. An
empty result is reported as an explicit "0 files matched scope" line, never an
empty stream.

### Progress and the status sidecar (`repo-scan-status/v1`)

`--progress` emits heartbeats to stderr. `--timeout-seconds <n>` and normal runs
write a status sidecar at `<out>.status.json` as a first-class artifact:

```json
{
  "schema_version": "repo-scan-status/v1",
  "scope": { "roots": ["."], "include": [], "exclude": [], "max_files": 50000 },
  "phase": "discovering | scanning | rendering | done | timed_out | terminated",
  "elapsed_seconds": 42,
  "discovered": 1280, "scanned": 1190, "remaining": 90,
  "cards": 17,
  "last_path": "crates/foo/src/raw.rs",
  "operator": { "state": "...", "next_action": "...", "claim_boundary": "..." }
}
```

The sidecar is written via write-then-rename so a reader never observes a torn
file. `last_path` and the discovered/scanned/remaining counts make a stalled or
slow scan diagnosable without attaching a debugger.

### Timeout, SIGTERM, SIGINT leave durable evidence

A run that hits `--timeout-seconds`, or receives SIGTERM/SIGINT, must leave
either durable artifacts (a finalized `<out>.partial` plus a status sidecar with
`phase: timed_out | terminated`) **or** a clear stderr diagnostic naming the
phase, elapsed time, and operator next step. The forbidden outcome is empty
stdout and empty stderr — a scan that "did nothing" with no trace. The
`<out>.partial` is produced write-then-rename so it is whole when observed.

### Direct file:line in reports

Repo-mode markdown anchors every card to its `file:line`, so an operator reading
a partial or completed report can navigate to the seam without a second lookup.

### The operator block

Every status sidecar and terminal diagnostic carries an operator block with
three fields: `state` (what the scan is/was doing), `next_action` (the single
concrete step — raise the timeout, narrow `--include`, inspect `last_path`), and
`claim_boundary` (what the artifact does and does not assert). The block is the
human-facing projection of the same trust boundary every surface ships
(UNSAFE-REVIEW-SPEC-0028); cross-tool consumers read it per
[`docs/interop/sibling-tools.md`](../interop/sibling-tools.md).

## Non-goals

This spec does not:

- change or widen analyzer detection, or add hazard families,
- block, gate, or change any exit code (movement and gating stay in
  UNSAFE-REVIEW-SPEC-0030; the orchestrator owns blocking),
- claim whole-repo coverage, proof, UB-free, Miri-clean, site-execution,
  calibrated precision/recall, or policy-readiness status,
- post comments, run witnesses, or edit source,
- redefine the coverage block (UNSAFE-REVIEW-SPEC-0029) or input scope
  (UNSAFE-REVIEW-SPEC-0003); it projects them across a tree.

## Trust boundary

The contract is a **bounded, diagnosable scan posture**, not a coverage
guarantee. A completed-file partial is a snapshot of the files scanned so far —
it is not full-repo coverage and asserts nothing about the unscanned remainder.
A `done` phase means every in-scope file was read under the stated filters, not
that the repository is memory-safe, UB-free, or Miri-clean. The status sidecar
and operator block report what the scan did; they make no safety claim.

## Proof obligations

- `cargo test -p unsafe-review-cli` — include/exclude/default-ignore/`--max-files`
  discovery selection; `--list-files` boundedness and honest-empty output;
  `repo-scan-status/v1` schema and write-then-rename atomicity.
- `cargo test -p unsafe-review` — acceptance smoke: a repo-mode run under a tight
  `--timeout-seconds` leaves a `phase: timed_out` sidecar plus a `<out>.partial`,
  and a SIGTERM/SIGINT run leaves a `phase: terminated` diagnostic — neither
  produces empty stdout and empty stderr.
- `cargo run --locked -p xtask -- check-pr`.

## Machine check

Registered in `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` and routed from
`docs/specs/UNSAFE-REVIEW-SPEC-START-HERE.md`; lifecycle and proof posture
validated by `cargo run --locked -p xtask -- check-spec-status` and
`cargo run --locked -p xtask -- check-docs`.
