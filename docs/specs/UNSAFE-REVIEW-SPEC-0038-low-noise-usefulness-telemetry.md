# UNSAFE-REVIEW-SPEC-0038: Low-Noise Usefulness Telemetry

Status: proposed
Owner: product / output
Created: 2026-06-12
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- plans/0.2.0/implementation-plan.md
Linked issues:
- none
Linked PRs:
- TBD
Support-tier impact: output projection only

## Problem

There is no machine-readable artifact that summarizes how well `unsafe-review`
surfaced actionable findings across the card, comment, and agent-readiness
surfaces for a given run. Without this, a consumer wanting to understand
diagnostic operational usefulness must re-derive counts from multiple artifacts
(`cards.json`, `comment-plan.json`, `unsafe-review-gate.json`) and manually
correlate fields.

## Trust boundary

This artifact is **operational diagnostic usefulness only**. It measures how well
the tool surfaces actionable findings across the card, comment, and agent-readiness
surfaces — not the quality or correctness of those findings. Specifically:

- This is **NOT** calibrated precision/recall (that is SPEC-0026).
- This does **NOT** claim memory-safety proof, not UB-free status, not Miri-clean
  status, and not a site-execution claim unless a matching witness receipt says so.
- This does **NOT** block, gate, or change exit codes.
- This does **NOT** run witnesses, post comments, or edit source.
- All telemetry is projected deterministically from existing
  ReviewCard/Summary/CoverageBlock/CommentPlan fields. The ReviewCard remains the
  single truth object; telemetry is a read-only projection layer.

## Artifact

`usefulness-telemetry.json` is emitted alongside all other `first-pr` artifacts.

### Schema version

`"usefulness-telemetry/v1"`

### Fields

#### `schema_version`

String. Always `"usefulness-telemetry/v1"`.

#### `trust_boundary`

String. Fixed advisory boundary text that must contain `"not calibrated"` and must
not contain `"precision"`, `"recall"`, `"proof"`, `"UB-free"`, `"Miri-clean"`, or
`"site-execution"` as positive claims.

#### `card_inventory`

Object. Derived from `Summary` fields.

| Field | Source field | Owning spec |
|---|---|---|
| `total_cards` | `Summary.cards` | SPEC-0029 |
| `actionable_cards` | `Summary.open_actionable_gaps` | SPEC-0029 |
| `new_cards` | `Summary.new_gaps` | SPEC-0030 |
| `worsened_cards` | `Summary.worsened_gaps` | SPEC-0030 |
| `improved_cards` | `Summary.improved_gaps` | SPEC-0030 |
| `resolved_cards` | `Summary.resolved_gaps` | SPEC-0030 |
| `inherited_cards` | `Summary.inherited_gaps` | SPEC-0030 |

`improved_cards` counts baseline-known cards whose evidence coverage improved
(at least one slot advanced, no slot regressed) since the baseline snapshot.
Always 0 until a baseline coverage snapshot exists.  An improved card is still
advisory, still open, still present — NOT resolved, NOT safe, NOT UB-free, NOT
Miri-clean, and NOT a site-execution claim.  See SPEC-0030 for the precedence
rule (worsened > improved > inherited).

#### `coverage_slots`

Object. Coverage slot counts derived from `CoverageBlock::derive(card)` per card.

| Field | Source | Owning spec |
|---|---|---|
| `contract_missing` | cards where `contract_coverage == Missing` | SPEC-0029 |
| `contract_weak` | always 0; contract has no weak state (included for schema completeness) | SPEC-0029 |
| `guard_missing` | cards where `guard_coverage == Missing` | SPEC-0029 |
| `guard_weak` | cards where `guard_coverage == Weak` | SPEC-0029 |
| `test_reach_missing` | cards where `test_reach_coverage == Missing` | SPEC-0029 |
| `test_reach_weak` | cards where `test_reach_coverage == Weak` | SPEC-0029 |
| `witness_receipt_missing` | cards where `witness_receipt_coverage == Missing` | SPEC-0029 |

#### `agent_readiness`

Object. Histogram derived from `CoverageBlock.agent_lsp_readiness` per card.
Must sum to `card_inventory.total_cards`.

| Field | Source | Owning spec |
|---|---|---|
| `ready` | cards where `agent_lsp_readiness == Ready` | SPEC-0029 |
| `requires_witness_receipt` | cards where `agent_lsp_readiness == RequiresWitnessReceipt` (class is RequiresLoom/RequiresSanitizer/RequiresKaniOrCrux — an external witness receipt is needed before repair delegation) | SPEC-0029 |
| `needs_human` | cards where `agent_lsp_readiness == NeedsHuman` | SPEC-0029 |
| `unsupported` | cards where `agent_lsp_readiness == Unsupported` | SPEC-0029 |

#### `comment_selection`

Object. Comment plan selection counts derived by re-rendering the comment plan
from the same `AnalyzeOutput` and parsing the JSON.

| Field | Source | Owning spec |
|---|---|---|
| `selected_count` | `CommentPlan.summary.selected_count` | SPEC-0022/0032 |
| `not_selected_count` | `CommentPlan.summary.not_selected_count` | SPEC-0022/0032 |
| `not_selected_reason_histogram` | histogram over `CommentPlan.not_selected[].reason_code`; only keys with count > 0 emitted | SPEC-0022/0032 |

#### `confidence_distribution`

Object. Confidence histogram over all cards.

| Field | Source | Owning spec |
|---|---|---|
| `high` | cards where `ReviewCard.confidence == High` | SPEC-0002 |
| `medium` | cards where `ReviewCard.confidence == Medium` | SPEC-0002 |
| `low` | cards where `ReviewCard.confidence == Low` | SPEC-0002 |
| `unknown` | cards where `ReviewCard.confidence == Unknown` | SPEC-0002 |

#### `actionability_distribution`

Object (BTreeMap). Histogram over all cards, keyed by actionability label.
Mirrors the `actionability()` logic in `output/comment_plan/selection.rs`.
Only keys with count > 0 are emitted.

Known keys: `specific_guard_missing`, `specific_contract_missing`,
`specific_witness_missing`, `specific_receipt_missing`, `specific_reach_missing`,
`human_review_only`, `not_actionable`.

#### `scan_cost` (SPEC-0038 §scan_cost)

Object. CLI-layer cost aperture injected by `render_usefulness_telemetry_with_cost`.
**Absent** (field omitted) when the renderer is called without cost context.

| Field | Source | Notes |
|---|---|---|
| `elapsed_ms` | `Instant` started before `analyze()` in CLI emit layer | Wall-clock ms; CLI layer only — core must not measure wall time |
| `output_bytes_total` | Running byte total accumulated across all artifact writes before telemetry file is written | Excludes the telemetry file itself (it is rendered before its own bytes are known) |

Trust boundary: diagnostic aperture only — not a coverage claim, proof,
UB-free, Miri-clean, site-execution, or performance guarantee.

#### `comment_selection.not_selected_class_histogram` (SPEC-0038 §not_selected_class_histogram)

Object (BTreeMap). Histogram of not-selected cards keyed by `"<reason_code>/<class>"`.
Allows consumers to distinguish a correct FFI/loom `lower_relevance` suppression
from an unactionable `budget_exhausted` miss.  Projected from
`CommentPlan.not_selected[].reason_code` and `.class`.
Only keys with count > 0 are emitted.

Example key: `"lower_relevance/ffi_boundary"`, `"budget_exhausted/raw_ptr_deref"`.

The sum of all values must equal the sum of all values in
`not_selected_reason_histogram` (same events, different keying).

#### `unfulfilled_obligation_count` (SPEC-0038 §unfulfilled_obligations)

Integer. Total count of per-obligation evidence slots across all cards where at
least one of contract/discharge/reach/witness is `present: false`.

This is a **work-surface signal** — a card with 5 obligations and none discharged
contributes 5, not 1.  A card with 5 obligations and 4 discharged contributes 1.
Use alongside `card_inventory.total_cards` to estimate per-card obligation depth.

Projected from `ReviewCard.obligation_evidence[].{contract,discharge,reach,witness}.present`.

Trust boundary: diagnostic aperture only — not a coverage claim, proof, UB-free,
Miri-clean, or site-execution claim.

### Gate manifest pointer

`unsafe-review-gate.json` gains an optional `artifacts.usefulness_telemetry` field
pointing to `"usefulness-telemetry.json"`. It is always set in `first-pr` runs.

## Non-goals

- This artifact does not measure analyzer quality or correctness.
- This artifact does not measure precision or recall (that is SPEC-0026).
- This artifact does not run agents, witnesses, or source edits.
- This artifact does not post comments.
- This artifact does not change exit codes or act as a gate.
- This artifact does not claim memory-safety proof, not UB-free status, not
  Miri-clean status, and not site-execution proof.

## Future fields (omitted, no clean deterministic source)

The following fields were considered and intentionally omitted:

| Field | Reason omitted |
|---|---|
| `cards_per_second` | Requires wall time; derivable from `scan_cost.elapsed_ms` and `card_inventory.total_cards` by the consumer |
| `time_to_first_useful_result` | No clean deterministic source |

The following fields were previously omitted but are now implemented:

| Field | Implemented in |
|---|---|
| `scan_cost.elapsed_ms` / `scan_cost.output_bytes_total` | SPEC-0038 §scan_cost via CLI injection pattern |
| `comment_selection.not_selected_class_histogram` | SPEC-0038 §not_selected_class_histogram |
| `unfulfilled_obligation_count` | SPEC-0038 §unfulfilled_obligations |

## Proof commands

```
cargo test -p unsafe-review-core usefulness_telemetry
cargo test -p unsafe-review --test e2e usefulness_telemetry
cargo run --locked -p xtask -- check-pr
```
