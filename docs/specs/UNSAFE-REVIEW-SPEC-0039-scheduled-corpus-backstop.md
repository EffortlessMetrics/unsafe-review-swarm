# UNSAFE-REVIEW-SPEC-0039: Scheduled Corpus Backstop and External Resource Harness

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
