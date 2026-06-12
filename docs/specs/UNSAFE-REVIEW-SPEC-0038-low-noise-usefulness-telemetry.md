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
| `resolved_cards` | `Summary.resolved_gaps` | SPEC-0030 |
| `inherited_cards` | `Summary.inherited_gaps` | SPEC-0030 |

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
| `scan_wall_ms` / `elapsed_ms` | No clean deterministic source in `AnalyzeOutput`; wall-clock would break reproducibility rails |
| `output_bytes` | Only available in CLI emit layer, not core output projection layer |
| `cards_per_second` | Requires wall time |
| `time_to_first_useful_result` | No clean deterministic source |

These may be added in a future revision when a clean, deterministic source exists
in `AnalyzeOutput` or when a separate timing aperture is standardized.

## Proof commands

```
cargo test -p unsafe-review-core usefulness_telemetry
cargo test -p unsafe-review --test e2e usefulness_telemetry
cargo run --locked -p xtask -- check-pr
```
