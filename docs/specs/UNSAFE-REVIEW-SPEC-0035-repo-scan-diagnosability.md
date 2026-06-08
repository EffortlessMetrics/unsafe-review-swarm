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

`--progress` emits heartbeats to stderr. `--timeout-seconds <n>`, `--max-cards
<n>`, and normal runs write a status sidecar at `<out>.status.json` as a
first-class artifact:

```json
{
  "schema_version": "repo-scan-status/v1",
  "scan_scope": {
    "root": ".",
    "include": [],
    "exclude": [],
    "respect_gitignore": true,
    "large_repo_ignores": true,
    "max_files": 50000
  },
  "phase": "discovering | scanning | complete | failed | terminated",
  "elapsed_ms": 42000,
  "files_discovered": 1280,
  "files_scanned": 1190,
  "files_remaining": 90,
  "cards_found": 17,
  "last_path": "crates/foo/src/raw.rs",
  "completed": true,
  "partial": false,
  "stop_reason": "none | max_cards | timeout | terminated",
  "cap": null,
  "error": null,
  "signal": null,
  "partial_path": null,
  "operator": {
    "state": "complete | capped | failed | terminated",
    "partial_report_available": false,
    "partial_report_limitation": "...",
    "next_action": "...",
    "claim_boundary": "..."
  }
}
```

**Phase vocabulary (shipped):**
- `discovering` — workspace file enumeration in progress.
- `scanning` — per-file analysis in progress.
- `complete` — scan finished reading all in-scope files (may be followed by
  `completed: true` for a full scan, or `partial: true` for a `--max-cards`
  capped scan that stopped before all files were read).
- `failed` — scan did not complete due to a timeout, an analysis error, or a
  report-write error.
- `terminated` — scan was interrupted by SIGTERM or SIGINT.

**Stop-reason vocabulary:**
- `none` — clean complete scan; every in-scope file was read.
- `max_cards` — `--max-cards N` cap was reached; scan stopped after N cards
  were emitted.  `cap` carries the configured N.
- `timeout` — `--timeout-seconds N` elapsed.
- `terminated` — a unix signal (SIGTERM / SIGINT) interrupted the scan.
- `error` — the scan did not complete due to an analysis error mid-scan or a
  report-write failure (anything on the `phase: "failed"` path that is not a
  timeout).  Timeout and error share the `failed` phase but are distinguished by
  `stop_reason` so a disk-write failure is never mislabeled as a timeout.

The `completed` field is `true` only for a full clean scan; a `max_cards`-capped
scan sets `completed: false` and `partial: true` even though `phase` is still
`complete` (the scan exited the file loop normally, just bounded).

The sidecar is written via write-then-rename so a reader never observes a torn
file. `last_path` and the discovered/scanned/remaining counts make a stalled or
slow scan diagnosable without attaching a debugger.

### Timeout, SIGTERM, SIGINT, and --max-cards leave durable evidence

A run that hits `--timeout-seconds`, receives SIGTERM/SIGINT, or reaches
`--max-cards` must leave either durable artifacts (a finalized report or
`<out>.partial`, plus a status sidecar with `stop_reason` and `partial: true`)
**or** a clear stderr diagnostic naming the phase, elapsed time, and operator
next step. The forbidden outcome is empty stdout and empty stderr — a scan that
"did nothing" with no trace. The `<out>.partial` is produced write-then-rename
so it is whole when observed.

Timeout surfaces as `phase: "failed"` + `stop_reason: "timeout"` + error
string in the status sidecar.  The `--max-cards` cap surfaces as `phase:
"complete"` + `completed: false` + `partial: true` + `stop_reason: "max_cards"`
+ `cap: N` — it is not a failure and exits 0.

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
- `cargo test -p unsafe-review` — acceptance smoke:
  - A repo-mode run under a tight `--timeout-seconds` leaves a `phase: "failed"` +
    `stop_reason: "timeout"` sidecar plus a `<out>.partial`; neither produces empty
    stdout and empty stderr.
  - A SIGTERM/SIGINT run leaves a `phase: "terminated"` + `stop_reason: "terminated"`
    sidecar; neither produces empty stdout and empty stderr.
  - A `--max-cards N` run over a multi-card tree leaves a final report with
    `partial: true`, `stop_reason: "max_cards"`, `cap: N`, `cards_found: N`, and
    a cap-specific operator `next_action`; exit code is 0 (not an error).
- `cargo test -p unsafe-review-core --lib` — pipeline unit test: `max_cards: Some(1)`
  over a multi-card tree emits a final status with `partial: true`,
  `stop_reason: MaxCards`, `cap: Some(1)`, `completed: false`.
- `cargo run --locked -p xtask -- check-pr`.

## Machine check

Registered in `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` and routed from
`docs/specs/UNSAFE-REVIEW-SPEC-START-HERE.md`; lifecycle and proof posture
validated by `cargo run --locked -p xtask -- check-spec-status` and
`cargo run --locked -p xtask -- check-docs`.
