# UNSAFE-REVIEW-SPEC-0039: Scheduled Corpus Backstop, Resource Harness, and Usefulness Rollup

Status: proposed
Owner: repo-infra
Created: 2026-06-12
Last touched: 2026-06-12

---

## Purpose

This spec defines a lightweight scheduled corpus backstop that runs the
fixture-control targets from `docs/dogfood/corpus.toml` on a weekly CI
schedule, collects wall-time, output size, and externally-measured peak RSS,
and uploads a `resource-report.json` artifact for triage purposes.

The backstop is a diagnostic signal surface, not a gate, not a coverage claim,
not a performance SLA, and not a memory-safety or UB-free proof.

---

## What runs

`xtask corpus-backstop` iterates every target in `docs/dogfood/corpus.toml`
with `kind = "fixture-control"` and `status = "active"`. For each target it:

1. Runs `unsafe-review check --root <root> --diff <diff> --format json --out <tmp>`.
2. Times execution with `std::time::Instant`.
3. Reads the output JSON to extract `output_bytes` (scan_status field if
   present), `files_discovered`, `files_scanned`, `files_skipped`, and
   `card_count` from the summary.
4. Records a run entry in the `runs[]` array.

After iterating all targets it writes a `resource-report.json` file at the
configured output path (default: `target/corpus-backstop/resource-report.json`).

The `peak_rss_bytes` field is always `null` when xtask runs locally. The CI
workflow injects the measured value via a post-process `jq` step (see
Workflow section below).

---

## `resource-report.json` schema

Schema version: `"0.1"`.

```json
{
  "schema_version": "0.1",
  "generated_at": "<ISO-8601 UTC timestamp>",
  "corpus_source": "docs/dogfood/corpus.toml",
  "run_summary": {
    "target_count": 2,
    "completed": 2,
    "failed": 0,
    "skipped": 0
  },
  "runs": [
    {
      "id": "<target id>",
      "kind": "fixture-control",
      "elapsed_ms": 312,
      "output_bytes": 4096,
      "files_discovered": 3,
      "files_scanned": 2,
      "files_skipped": 1,
      "card_count": 1,
      "status": "completed"
    }
  ],
  "totals": {
    "elapsed_ms_total": 312,
    "output_bytes_total": 4096,
    "card_count_total": 1
  },
  "peak_rss_bytes": null,
  "peak_rss_source": null,
  "trust_boundary": "Corpus backstop report is diagnostic triage input only, not a coverage, precision, recall, memory-safety, UB-free, Miri-clean, site-execution, or performance SLA claim; not a gate."
}
```

### Field definitions

| Field | Type | Description |
|---|---|---|
| `schema_version` | string | Always `"0.1"` |
| `generated_at` | string | ISO-8601 UTC timestamp |
| `corpus_source` | string | Source manifest path |
| `run_summary.target_count` | number | Total fixture-control targets attempted |
| `run_summary.completed` | number | Runs that finished without error |
| `run_summary.failed` | number | Runs that returned a non-zero exit code |
| `run_summary.skipped` | number | Targets skipped (inactive or wrong kind) |
| `runs[].id` | string | Target id from corpus.toml |
| `runs[].kind` | string | Target kind (always `"fixture-control"` for these runs) |
| `runs[].elapsed_ms` | number | Wall time in milliseconds |
| `runs[].output_bytes` | number | Output JSON byte count, or 0 if unavailable |
| `runs[].files_discovered` | number | Files discovered by the scan |
| `runs[].files_scanned` | number | Files actually scanned |
| `runs[].files_skipped` | number | Files skipped by the scan |
| `runs[].card_count` | number | Number of review cards produced |
| `runs[].status` | string | `"completed"` or `"failed"` |
| `totals.elapsed_ms_total` | number | Sum of all `elapsed_ms` |
| `totals.output_bytes_total` | number | Sum of all `output_bytes` |
| `totals.card_count_total` | number | Sum of all `card_count` |
| `peak_rss_bytes` | number \| null | Peak RSS in bytes; null when not externally provided |
| `peak_rss_source` | string \| null | Description of the RSS measurement method; null when not provided |
| `trust_boundary` | string | Fixed diagnostic boundary statement |

---

## External RSS mechanism

`peak_rss_bytes` is always `null` when xtask runs locally. The scheduled CI
workflow injects it by running the entire backstop command under
`/usr/bin/time -v` (available on Linux runners), parsing the
`"Maximum resident set size (kbytes):"` line from stderr, converting kbytes to
bytes, and rewriting the report with a `jq` post-process step:

```bash
RSS_KB=$(grep "Maximum resident set size" time-output.txt | grep -oE '[0-9]+' | tail -1)
RSS_BYTES=$((RSS_KB * 1024))
jq --argjson rss "$RSS_BYTES" \
  '.peak_rss_bytes = $rss | .peak_rss_source = "/usr/bin/time -v on Linux ubuntu-latest runner"' \
  resource-report.json > resource-report.json.tmp && mv resource-report.json.tmp resource-report.json
```

This mechanism is entirely external to the tool. The tool never attempts to
measure its own RSS.

---

## Workflow

The scheduled workflow `.github/workflows/corpus-backstop.yml` runs on:

- `schedule: '0 4 * * 1'` (weekly, Monday 04:00 UTC)
- `workflow_dispatch`

It is **never** a required PR gate. It does not trigger on `pull_request` or
`push`. It uploads `target/corpus-backstop/` as an artifact with 30-day
retention.

---

## Schema validation

`xtask check-corpus-backstop-schema <path>` validates a `resource-report.json`
file against the schema contract. It checks:

- `schema_version` exists and is a string.
- `generated_at` exists and is a non-empty string.
- `runs` is an array.
- `trust_boundary` exists and contains `"diagnostic triage input only"` and
  `"not a gate"`.
- `peak_rss_bytes` is either `null` or a positive number.
- Each entry in `runs[]` has: `id` (string), `kind` (string),
  `elapsed_ms` (number), `status` (string).

The sample file `policy/corpus-backstop-sample-report.json` is validated by
`check-pr` via `check_policy()` without requiring a live run, ensuring the
schema contract stays machine-checked on every PR.

---

## `xtask corpus-backstop` subcommand

```
cargo run --locked -p xtask -- corpus-backstop [--out <path>]
```

- Default output: `target/corpus-backstop/resource-report.json`
- Exits 0 even if some targets fail; failed targets are recorded in
  `runs[].status = "failed"` and counted in `run_summary.failed`.
- Does not fail CI; the workflow uses `|| true` after this command.

---

## Non-goals

- No long-term archival or trend visualization.
- No in-tool RSS measurement; RSS injection is entirely external.
- No blocking or SLA enforcement of any kind.
- No precision, recall, coverage, memory-safety, UB-free, Miri-clean, or
  site-execution claim.
- No new analyzer behavior; fixture-controls only.

---

## Trust boundary

**Diagnostic triage input only.**

The corpus backstop report is diagnostic triage input only, not a coverage,
precision, recall, memory-safety, UB-free, Miri-clean, site-execution, or
performance SLA claim; not a gate. Peak RSS is measured externally and
is a rough triage signal, not a calibrated or reproducible memory-usage
benchmark. Elapsed times reflect CI runner variability and are not a
performance contract.

---

## Part 2: Corpus Usefulness Rollup

### Purpose

This section extends SPEC-0039 with a corpus-wide usefulness/noise rollup
that aggregates SPEC-0038 `usefulness-telemetry.json` output across a bounded,
documented representative subset of local `fixtures/`.

This fills a validation gap: `usefulness-telemetry.json` is emitted per-run
but was never aggregated, so "is it low-noise across the corpus?" could not be
answered without manual correlation.

SPEC-0039 is the natural home (not SPEC-0038) because: SPEC-0038 owns the
per-run telemetry schema; SPEC-0039 already owns the scheduled corpus harness
pattern and off-PR-path running semantics. This is an additive second section
of the same spec — not a change to SPEC-0038.

### What runs

`xtask corpus-usefulness` runs `unsafe-review first-pr --root <fixture>
--diff <fixture>/change.diff --out-dir <tmp>` for each fixture in a curated,
documented representative subset of `fixtures/`. It reads the emitted
`usefulness-telemetry.json` from each run and aggregates the SPEC-0038 fields.

The subset is bounded (15-25 fixtures, documented in
`xtask/src/corpus_usefulness.rs:SUBSET`) and selected to span the noise shapes:

| Noise shape | Example fixture |
|---|---|
| Negative control (zero cards) | `safe_code_no_cards`, `adjacent_unchanged_unsafe_fn_no_card` |
| Single-gap positive | `raw_pointer_alignment` |
| Witnessed / receipt attached | `raw_pointer_alignment_receipted` |
| False-positive control | `raw_pointer_alignment_closed_branch_not_guard` |
| Multi-gap / multi-obligation | `vec_from_raw_parts`, `vec_from_raw_parts_manuallydrop_origin` |
| Capped / multi-pointer | `copy_nonoverlapping` |
| Multi-pointer with full guards | `copy_nonoverlapping_slice_range_guard` |
| Alternate operation families | `box_from_raw`, `drop_in_place_deallocation`, `transmute_bool_disjunct_return_guard` |
| Human-review-only | `inline_asm_human_review` |
| FFI boundary | `ffi_missing_boundary_contract`, `ffi_sanitizer_route` |
| Atomic / agent-ready shape | `atomic_pointer_state_swap` |
| Contract coverage shape | `documented_private_unsafe_fn` |

### `corpus-usefulness-rollup.json` schema

Schema version: `"corpus-usefulness-rollup/v1"`.

| Field | Type | Description |
|---|---|---|
| `schema_version` | string | Always `"corpus-usefulness-rollup/v1"` |
| `generated_at` | string | ISO-8601 UTC timestamp |
| `trust_boundary` | string | Fixed advisory boundary — must contain `"not calibrated"` and `"not a gate"` |
| `fixture_subset` | array | Per-fixture entries: `fixture`, `rationale`, `status`, `elapsed_ms` |
| `corpus_totals.fixtures_run` | number | Total fixtures attempted |
| `corpus_totals.fixtures_completed` | number | Fixtures that completed without error |
| `corpus_totals.fixtures_failed` | number | Fixtures that failed or were skipped |
| `card_inventory` | object | Corpus totals for `total_cards`, `actionable_cards`, `new_cards`, `worsened_cards`, `resolved_cards`, `inherited_cards` |
| `coverage_slots` | object | Corpus totals for SPEC-0029 coverage slot fields |
| `agent_readiness` | object | Corpus histogram for `ready`, `requires_witness_receipt`, `needs_human`, `unsupported` |
| `not_selected_reason_histogram` | object (BTreeMap) | Corpus histogram of SPEC-0038 not-selected reason codes |
| `not_selected_class_histogram` | object (BTreeMap) | Corpus histogram of `reason/class` keys |
| `unfulfilled_obligation_count` | number | Corpus total unfulfilled obligation slots |
| `scan_cost_range.elapsed_ms_min/median/max` | number \| null | Distribution of CLI-measured elapsed times across completed runs |
| `scan_cost_range.output_bytes_min/median/max` | number \| null | Distribution of output bytes where telemetry `scan_cost` is present |
| `human_summary` | string | Single human-readable summary line |

### Trust boundary

Diagnostic noise/usefulness characterisation only.

The rollup is NOT calibrated precision or recall, NOT a coverage claim, NOT a
memory-safety proof, NOT UB-free, NOT Miri-clean, NOT site-execution, NOT a
gate. Aggregated from SPEC-0038 `usefulness-telemetry.json` projected from
ReviewCard truth objects across the listed fixture subset.

### Off-PR-path placement

`corpus-usefulness` is NEVER on the per-PR critical path. Running 15-25
`first-pr` invocations is too slow for `check-pr`. Only a fast schema check
on the committed sample rollup (`policy/corpus-usefulness-sample-rollup.json`)
belongs in `check-pr`, via `check_policy()`.

### Schema validation

`xtask check-corpus-usefulness-schema <path>` validates a
`corpus-usefulness-rollup.json` against the contract. It checks:

- `schema_version` is `"corpus-usefulness-rollup/v1"`.
- `generated_at` is a non-empty string.
- `trust_boundary` contains `"not calibrated"` and `"not a gate"` and does
  not make positive `UB-free`, `Miri-clean`, `site-execution`, or `proof` claims.
- `fixture_subset` is a non-empty array of objects with `fixture` and `rationale`.
- `corpus_totals` is an object with numeric `fixtures_run`, `fixtures_completed`,
  `fixtures_failed`.
- `card_inventory` is an object.
- `agent_readiness` is an object.
- `scan_cost_range` has `elapsed_ms_min`, `elapsed_ms_median`, `elapsed_ms_max`
  (each number or null).
- `human_summary` is a non-empty string.

The sample file `policy/corpus-usefulness-sample-rollup.json` is validated by
`check-pr` via `check_policy()` without requiring a live run.

### `xtask corpus-usefulness` subcommand

```
cargo run --locked -p xtask -- corpus-usefulness [--out <path>]
```

- Default output: `target/corpus-usefulness/corpus-usefulness-rollup.json`
- Exits 0 even if some fixtures fail; failures are recorded in
  `fixture_subset[].status = "failed"`.
- Never used in CI gate; intended for local diagnostic validation and
  scheduled off-PR harness runs.
