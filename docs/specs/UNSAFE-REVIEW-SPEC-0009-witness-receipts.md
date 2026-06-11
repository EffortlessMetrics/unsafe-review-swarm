# UNSAFE-REVIEW-SPEC-0009: Witness receipts

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md
Linked issues:
- EffortlessMetrics/unsafe-review-swarm#1602 (WitnessMismatch is_actionable fix)

## Problem

`unsafe-review` needs a precise, checkable behavior contract for witness receipts.

## Behavior

Receipts record configured, ran, test-targeted, or site-reached witness strength
and limitations. The current implementation imports JSON receipts from:

```text
.unsafe-review/receipts/*.json
```

Each receipt is matched by exact counted `card_id`. A matching receipt marks the
card's top-level witness evidence present and marks obligation-level witness
evidence present only when the receipt tool matches one of the current card's
routed witness tools and the receipt strength records a saved witness run
(`ran`, `test_targeted`, or `site_reached`) or human-review-only `reviewed`
strength. A `configured`, expired, wrong-tool, or non-human `reviewed` receipt
remains valid receipt metadata for audit or is rejected by validation as
specified below, but it does not remove the missing witness gap. Card-level
witness summaries should surface same-card metadata-only state so reviewers can
see why a saved receipt did not import as current witness evidence.

When an otherwise-sound card (contract, guard, and reach evidence all present)
has an imported receipt whose `tool` does not match any of the card's routed
witness tools, the card is classified as `WitnessMismatch` rather than
`GuardedUnwitnessed`. A tool mismatch is a live, surfaced condition — a saved
receipt exists but it cannot satisfy the current route — so
`ReviewClass::is_actionable()` returns `true` for `WitnessMismatch`. This means
`WitnessMismatch` cards feed the same downstream surfaces as any other open
actionable class:

- **LSP diagnostics** — a `warning`-severity diagnostic is emitted (same rule
  as all actionable classes in `lsp/diagnostics.rs`).
- **Baseline state** — `BaselineState::New` is produced by `CoverageBlock` so
  the card participates in no-new-debt accounting.
- **Policy report** — `PolicyStatus::NewGap` is produced by `policy_report.rs`
  so the card increments the new-debt counter.
- **Agent/LSP readiness** — `agent_lsp_readiness = "ready"` is set by
  `agent/readiness.rs`.
- **Comment plan** — `comment_plan/selection.rs` considers the card eligible
  for the `specific_receipt_missing` bucket.
- **Summary action count** — `pipeline/summary.rs` counts the card in the open
  actionable set.
- **API** — `api.rs` exposes the card in the actionable gap list.

Rule: any change to the `is_actionable()` match arm is cross-surface by
construction. A new variant added to or removed from that arm must update tests
in `lsp/tests.rs`, `output/policy_report.rs`, and `domain/coverage.rs` (the
baseline-state and outcome-movement drift-locks) so regressions are caught at
compile time, not at customer sites. Receipt
import does not discharge contracts or guards. Receipt import does not discharge
reach evidence except for the explicitly reach-only
`tool = "external-integration-test"` / `strength = "site_reached"` receipt
described below.

The receipt shape is represented in the core SDK as the serde-backed
`WitnessReceipt` DTO. Importers and future native adapters must use that same
shape instead of inventing parallel receipt schemas.

Receipt `command_hash` is an optional stable command-string fingerprint. It is
for drift detection and reviewer comparison only; it is not cryptographic proof
and does not prove that the command ran. Older receipts without `command_hash`
remain importable. When `command_hash` is present, validation checks that it
matches the exact `command` string in the same receipt.

The CLI may render a receipt template from explicit user-provided metadata. That
template output is only a JSON authoring aid; it must not run witness commands or
claim that a witness succeeded.

For reviewed C++ or other foreign FFI seams, a `human-deep-review` receipt may
record that a reviewer checked the current Rust extern declaration against the
cited foreign declaration or ownership contract. It imports as witness evidence
with `strength = "reviewed"` only when the current ReviewCard routes
`human-deep-review` and the receipt matches the exact counted card identity. It
does not execute code, does not discharge contract, guard, or reach evidence,
and does not prove the foreign side or repository safe. If the extern
declaration changes enough to change the ReviewCard identity, the old receipt
becomes stale metadata until reviewed again.

For Rust unsafe seams reached through another language's integration suite, an
`external-integration-test` receipt may record the external command and reviewer
summary for an exact current ReviewCard identity. It imports as reach evidence
with `strength = "site_reached"` only when the receipt matches the exact counted
card identity, is current, and has no duplicate reach-importing receipt for that
card. It does not import witness evidence, does not execute the external test,
does not independently prove site execution, does not discharge contract or
guard evidence, and does not prove memory safety. The `command` field is
required for this tool so the external harness remains auditable. If the
ReviewCard identity changes, the old receipt becomes stale metadata until the
external reach evidence is reviewed again.

The CLI may import a receipt from saved Miri output. The adapter must read an
existing log file, reject empty output, reject failure-looking output, require
`test result: ok`, and emit a normal `WitnessReceipt` with `tool = "miri"` and
`strength = "ran"`. It must not execute Miri, infer site reach, or create a
card.

The CLI may import a receipt from saved `cargo-careful` output. The adapter must
read an existing log file, reject empty output, reject failure-looking output,
require `test result: ok`, and emit a normal `WitnessReceipt` with
`tool = "cargo-careful"` and `strength = "ran"`. It must not execute
`cargo-careful`, infer site reach, or create a card.

The CLI may import a receipt from saved sanitizer output. The adapter must read
an existing log file, require an explicit sanitizer `tool` of `asan`, `msan`,
`tsan`, or `lsan`, reject empty output, reject failure-looking output, require
`test result: ok`, and emit a normal `WitnessReceipt` with that sanitizer tool
and `strength = "ran"`. It must not execute a sanitizer, infer site reach, or
create a card.

The CLI may import a receipt from saved Loom or Shuttle output. The adapter must
read an existing log file, require an explicit concurrency `tool` of `loom` or
`shuttle`, reject empty output, reject failure-looking output, require
`test result: ok`, and emit a normal `WitnessReceipt` with that concurrency tool
and `strength = "ran"`. It must not execute Loom or Shuttle, infer site reach,
or claim complete scheduler coverage.

The CLI may import a receipt from saved Kani or Crux proof output. The adapter
must read an existing log file, require an explicit proof `tool` of `kani` or
`crux`, reject empty output, reject failure-looking output, require a
conservative verification-success marker, and emit a normal `WitnessReceipt`
with that proof tool and `strength = "ran"`. It must not execute Kani or Crux,
infer site reach, create a card, or claim proof beyond the recorded
harness/output scope.

The CLI may also validate receipt files without running analysis. Validation must
use the same importer checks as normal card analysis so users do not get a
separate receipt truth.

The CLI may audit receipt files against the current `ReviewCard` set. Receipt
audit must report matched, unmatched, stale, expired, wrong-identity,
wrong-tool, weaker-than-required, command-hash-mismatch, duplicate, and invalid
receipt metadata without running witnesses or external integration tests,
making policy decisions, or claiming safety. Audit must also mark the subset of
receipts that would import as current ReviewCard witness evidence with
`imports_witness_evidence`; this requires a current card match, a routed tool,
importable run strength (`ran`, `test_targeted`, or `site_reached`) or
human-review `reviewed` strength, no expiry, no validation error, and no
duplicate witness-importing receipt for the same card. Audit must mark the
subset of receipts that would import as current ReviewCard reach evidence with
`imports_reach_evidence`; this requires a current card match,
`tool = "external-integration-test"`, `strength = "site_reached"`, no expiry,
no validation error, and no duplicate reach-importing receipt for the same
card. Matched receipt
entries include current card operation, missing-count, and next-action context.
Audit entries include the current card's routed witness tools so a reviewer can
compare the saved receipt tool against the ReviewCard route. Audit entries also
include the saved
`summary`, saved `author`, saved `recorded_at` timestamp, saved `command_hash`
when present, saved per-receipt limitations, and surface command-hash
mismatches as their own audit status so reviewers can compare receipt synopsis,
ownership, recency, command-string drift, and saved scope limits without
treating any of them as proof that the command ran or covered the unsafe site.
When a matching receipt is imported as ReviewCard witness evidence, the
evidence summary also includes the saved `command_hash` when present so
card-level projections keep the same drift key visible.
This lets a receipt improve witness or external reach evidence without erasing
remaining guard, contract, witness, or reach gaps outside that exact evidence
kind. The audit is an advisory metadata report over saved receipts and current
cards. JSON and Markdown audit output must include explicit limitations saying
the audit uses saved metadata only, does not execute witness tools or external
integration tests, does not independently prove site reach or safety, and does
not erase remaining contract, guard, witness, or reach gaps outside the imported
evidence kind.

Receipt JSON fields:

```json
{
  "schema_version": "0.1",
  "card_id": "UR-...-c1",
  "tool": "miri",
  "strength": "ran",
  "author": "core/fixtures",
  "recorded_at": "2026-05-18T00:00:00Z",
  "expires_at": "2026-08-18",
  "summary": "focused witness passed",
  "command": "cargo +nightly miri test read_header",
  "command_hash": "3e163b0bce29ff2e",
  "limitations": ["fixture only"],
  "verdict": "not_reproduced"
}
```

`verdict` is optional. When present it must be one of:

- `confirmed`
- `not_reproduced`
- `inconclusive`

"confirmed" means the UB-risk hypothesis reproduced; "not_reproduced" means
this single run did not reproduce it — it is NOT a safety claim.
`inconclusive` marks an ambiguous or partial run. Receipts written before
this field existed omit it and stay valid; unknown values are rejected. The
saved-output import constructors record `not_reproduced` because they only
accept clean targeted runs; they reject outputs carrying hazard signals, so
`confirmed` is never derivable from them.

`strength` must be one of:

- `configured`
- `ran`
- `test_targeted`
- `site_reached`
- `reviewed` (only for `tool = "human-deep-review"`)

`tool` must be one of the supported receipt lanes:

- `miri`
- `cargo-careful`
- `asan`
- `msan`
- `tsan`
- `lsan`
- `loom`
- `shuttle`
- `kani`
- `crux`
- `human-deep-review`
- `external-integration-test`
- `unsupported`

`author` must be non-empty. `recorded_at` must be a calendar-valid UTC
timestamp in `YYYY-MM-DDTHH:MM:SSZ` form. `expires_at` must be a
calendar-valid `YYYY-MM-DD` date on or after the `recorded_at` date.
`external-integration-test` receipts must use `strength = "site_reached"` and
must include a non-empty `command`.

## Non-goals

- no soundness claim
- no hidden blocking unless policy mode explicitly enables it
- no duplicate truth outside this spec and linked policy files
- no witness execution by `unsafe-review` by default; the explicit
  `confirm --allow-heavy` opt-in executes one routed witness command locally
  and records the result only as a saved witness receipt through the existing
  saved-output import constructors, with no success claim without a receipt;
  the template and `import-*` commands still never execute anything
- no receipt match without exact card identity
- no claim that a receipt proves arbitrary callers or the whole repository safe

## Required evidence

- fixture-backed examples for positive and negative cases
- JSON output contract coverage
- analyzer tests for exact receipt import
- receipt parser tests for strength and identity validation
- policy documentation when behavior is configurable

## Acceptance examples

- A matching routed-tool `ran`, `test_targeted`, `site_reached`, or
  human-review-only `reviewed` receipt removes the `witness` missing-evidence
  item.
- A matching routed-tool `ran`, `test_targeted`, `site_reached`, or
  human-review-only `reviewed` receipt marks obligation-level witness evidence
  present.
- A matching `human-deep-review` receipt for an FFI card removes only the
  witness missing-evidence item and leaves contract, guard, and reach gaps
  visible when they remain statically unresolved.
- A matching `external-integration-test` receipt removes only the `reach`
  missing-evidence item and leaves contract, guard, and witness gaps visible
  when they remain unresolved.
- A matching routed witness receipt and a matching external integration reach
  receipt may coexist for the same ReviewCard identity.
- A `reviewed` receipt whose tool is not `human-deep-review` is rejected.
- An `external-integration-test` receipt whose strength is not `site_reached`
  is rejected.
- An `external-integration-test` receipt without a command is rejected.
- A matching `configured` receipt validates and appears in receipt audit, but
  does not remove the `witness` missing-evidence item.
- A matching wrong-tool receipt validates and appears in receipt audit, but does
  not remove the `witness` missing-evidence item.
- A card whose contract, guard, and reach evidence are all present, but whose
  only imported receipt has a tool that does not match any routed witness tool,
  is classified as `WitnessMismatch` (not `GuardedUnwitnessed`).
- A `WitnessMismatch` card is actionable: it produces a `warning`-severity LSP
  diagnostic, `BaselineState::New`, `PolicyStatus::NewGap`, and is included in
  the open actionable set reported to the agent and comment plan surfaces
  (issue #1602).
- A receipt with unknown tool is rejected.
- A receipt with unknown strength is rejected.
- A receipt with uncounted card identity is rejected.
- A receipt missing author, timestamp, or expiry metadata is rejected.
- A receipt with a shaped but invalid calendar date is rejected.
- A receipt whose expiry predates its recorded date is rejected.
- If receipt scope is limited, the receipt summary keeps that limitation visible.
- The core `WitnessReceipt` DTO round-trips through serde JSON and validates the
  same required fields as the importer.
- The CLI receipt-template command writes a valid receipt JSON object but does
  not execute the recorded command.
- The CLI Miri saved-output adapter writes a receipt from a success-looking Miri
  log and rejects failure-looking output.
- The CLI `cargo-careful` saved-output adapter writes a receipt from a
  success-looking `cargo-careful` log and rejects failure-looking output.
- The CLI sanitizer saved-output adapter writes a receipt from a success-looking
  sanitizer log, rejects unsupported sanitizer tools, and rejects
  failure-looking output.
- The CLI concurrency saved-output adapter writes a receipt from a
  success-looking Loom/Shuttle log, rejects unsupported concurrency tools, and
  rejects failure-looking output.
- The CLI proof saved-output adapter writes a receipt from a success-looking
  Kani/Crux log, rejects unsupported proof tools, and rejects failure-looking
  output.
- The CLI receipt-validate command counts importable receipts and rejects the
  same invalid receipt files as normal analysis.
- The CLI receipt-audit command reports matched, stale, expired,
  wrong-identity, wrong-tool, weaker-than-required, command-hash-mismatch,
  duplicate, and invalid receipts without executing witnesses or making policy
  decisions.
- The CLI receipt-audit command marks only currently importable saved witness
  receipts with `imports_witness_evidence`; matching `configured`,
  wrong-tool, expired, invalid, or duplicate receipts remain audit metadata.
- Receipt-audit JSON and Markdown include per-receipt `summary`, `author`,
  `recorded_at`, and limitation metadata plus report limitations that preserve
  the saved metadata boundary and state that matched receipts only improve
  witness evidence.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
cargo test -p unsafe-review-core receipt
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
