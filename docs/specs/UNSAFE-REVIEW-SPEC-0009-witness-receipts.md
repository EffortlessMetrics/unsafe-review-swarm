# UNSAFE-REVIEW-SPEC-0009: Witness receipts

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md

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
evidence present. Receipt import does not discharge contracts, guards, or reach
evidence.

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
receipt metadata without running witnesses, inferring site reach, making policy
decisions, or claiming safety. Matched receipt entries include current card
operation, missing-count, and next-action context. Audit entries include the
current card's routed witness tools so a reviewer can compare the saved receipt
tool against the ReviewCard route. Audit entries also include the saved
`command_hash` when present and surface command-hash mismatches as their own
audit status so reviewers can compare command-string drift without treating it
as proof that the command ran.
When a matching receipt is imported as ReviewCard witness evidence, the
evidence summary also includes the saved `command_hash` when present so
card-level projections keep the same drift key visible.
This lets a receipt improve witness evidence without erasing remaining guard or
contract gaps. The audit is an advisory metadata report over saved receipts and
current cards. JSON and Markdown audit output must include explicit limitations
saying the audit uses saved metadata only, does not execute witness tools, does
not prove site reach or safety, and does not erase remaining contract, guard, or
reach gaps.

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
  "limitations": ["fixture only"]
}
```

`strength` must be one of:

- `configured`
- `ran`
- `test_targeted`
- `site_reached`

`tool` must be one of the supported witness lanes:

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
- `unsupported`

`author` must be non-empty. `recorded_at` must be a calendar-valid UTC
timestamp in `YYYY-MM-DDTHH:MM:SSZ` form. `expires_at` must be a
calendar-valid `YYYY-MM-DD` date on or after the `recorded_at` date.

## Non-goals

- no soundness claim
- no hidden blocking unless policy mode explicitly enables it
- no duplicate truth outside this spec and linked policy files
- no witness execution by `unsafe-review`
- no receipt match without exact card identity
- no claim that a receipt proves arbitrary callers or the whole repository safe

## Required evidence

- fixture-backed examples for positive and negative cases
- JSON output contract coverage
- analyzer tests for exact receipt import
- receipt parser tests for strength and identity validation
- policy documentation when behavior is configurable

## Acceptance examples

- A matching receipt removes the `witness` missing-evidence item.
- A matching receipt marks obligation-level witness evidence present.
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
- Receipt-audit JSON and Markdown include limitations that preserve the saved
  metadata boundary and state that matched receipts only improve witness
  evidence.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
cargo test -p unsafe-review-core receipt
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
