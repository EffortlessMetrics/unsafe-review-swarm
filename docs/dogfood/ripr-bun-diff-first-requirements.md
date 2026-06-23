# ripr Bun Diff-First Requirements

Status: future tooling-interface requirements

**Producer-side contract.** The ripr producer-side contract for this rail is
SPEC-0070 (`docs/specs/RIPR-SPEC-0070-downstream-review-consumer-use-case.md`
in ripr-swarm, merged via ripr-swarm#1046). SPEC-0070 §"Rail alignment" maps
every requirement in this document to a concrete ripr field or a named gap.
See issue EffortlessMetrics/ripr-swarm#1041 for alignment history. A revision
to this rail without a corresponding SPEC-0070 update is a named defect ("rail
drift" per SPEC-0070 §"Failure Modes").

**Gap ownership (2026-06-23 consumer-side review).** The five named gaps from
SPEC-0070 have been reviewed from the consumer side. The ownership decisions
are recorded in §"Gap ownership decisions" at the end of this document. No gap
may be silently dropped.

This note records what the Bun stable-byte burndown needs from `ripr` when it
acts as a diff-first inventory or mutation-exposure helper for unsafe-review.
It is a requirements rail only. It does not add a live integration, run
mutation tooling, execute witnesses, edit source, post comments, or turn any
finding into a policy gate.

## Problem

Bun-scale fork deltas are large enough that a full repository seam cache can
block the useful path from changed code to candidate packet. The current
throughput failure mode is a large-entry skip such as:

```text
skipped_large_entry_seams_411564_limit_20000
```

That kind of skip is especially costly when the human lane is investigating a
small PR or two-file diff: the useful output is the changed seam first, not a
complete whole-repo inventory.

## Requirements

`ripr` support for Bun should prefer changed seams before broad inventory:

- accept a repository root and PR/fork diff as first-class input;
- rank changed files, changed hunks, and changed unsafe/native seams before
  whole-repo seam cache work;
- emit usable partial output before broad cache completion;
- record skipped broad-cache work without returning a zero-byte or empty
  "success" result;
- persist cache entries by source file hash, tool version, and scan mode so a
  repeated Bun fork scan can reuse prior inventory;
- preserve mixed-language route context when the oracle is JavaScript or
  TypeScript but the seam is Rust, Zig, C, C++, or native FFI;
- make large-repo skip remediation explicit and tool-supported rather than
  inventing command flags in downstream docs.

When broad inventory is skipped, the output should still answer:

- which changed Rust/native seams were inspected;
- which changed seams were skipped;
- why the skip happened, including the numeric limit and observed count;
- what exact supported command, option, cache setting, or narrower scope would
  unblock the scan;
- whether the result is partial, complete, or unavailable.

## Output Shape

The Bun lane needs machine-readable output shaped like:

```yaml
schema_version: ripr-bun-diff-first/v1
mode: diff_first
status: partial
root: /path/to/bun
diff: /path/to/change.diff
changed_seams_first: true
large_repo_skip:
  reason: skipped_large_entry_seams
  observed: 411564
  limit: 20000
  remediation: exact supported ripr command or option goes here
seams:
  - rust_seam: src/runtime/api/BunObject.rs::gzip_or_deflate_sync
    source_route: Bun.gzipSync BufferSource plus options getter reentry
    stable_byte_family: stable-byte-source-getter-reentry
    proof_mode: observable-red-green
    oracle_language: typescript
    oracle_path: test/js/bun/util/compression-getter-reentry.test.ts
    oracle_kind: stale-span-red-green
    coverage_confidence: candidate-local
    limitation: route and oracle map only; not witness execution or proof
```

The exact schema can change when `ripr` exists as a checked integration, but the
fields above are the minimum Bun control-plane data that downstream packets need
to stay useful without overclaiming.

## Receipts

Any future `ripr` receipt is external evidence for inventory or mutation
exposure only. It must say what was scanned, what was skipped, what cache was
used, and what command or tool version produced the output.

A `ripr` receipt remains external evidence only. It is:

- not witness execution;
- not Miri-clean evidence;
- not site-execution proof;
- not proof of UB;
- not proof of memory safety;
- not a calibrated precision/recall claim;
- not a default blocking policy.

For manual candidates, `ripr` evidence may help preserve a route, seam map, or
oracle map, but it must keep `source = manual`, `manual_candidate = true`, and
`analyzer_discovered = false` unless a separate reviewed analyzer linkage exists.

## Acceptance Checklist

A future implementation should be accepted only when:

- a two-file Bun diff can produce changed-seam output before whole-repo cache
  completion;
- large-entry skip output includes count, limit, scope, and remediation;
- partial artifacts are non-empty and carry status metadata;
- cache persistence is explicit and reproducible;
- cross-language oracle fields are present for JS/TS tests that map to Rust or
  native seams;
- receipts preserve inventory limits and do not claim witness or proof status.

## Trust Boundary

This document is a future tooling requirement. It is not an implementation
receipt, not analyzer discovery, not witness execution, not source editing, not
automatic commenting, not proof of memory safety, not UB-free status, not
Miri-clean status, not site-execution proof, not calibrated precision or recall,
and not policy readiness.

## Gap ownership decisions

Consumer-side review of SPEC-0070 §"Rail alignment" (2026-06-23). Each gap is
confirmed accurate from the consumer side. The ownership decision for each is
one of: **ripr-side closure** (ripr adds a field), **consumer-side closure**
(unsafe-review synthesises downstream), or **deferred** (stays named in both
documents until a slice closes it).

### Rail alignment table review

The SPEC-0070 rail alignment table was reviewed row-by-row against this
document's Requirements, Output Shape, Receipts, and Acceptance Checklist
sections. **All mappings are confirmed accurate from the consumer side.** No
row misrepresents the rail's intent. Specifically:

- `schema_version`: confirmed — the consumer expects `"0.1"` on check JSON,
  evidence records, and OUTPUT_SCHEMA.md.
- `status: partial` semantics: confirmed — `limited_*` + `downstream_consumable
  = false` for whole-repo; `limited_diff_scope` + `downstream_consumable = true`
  for diff-scoped matches the rail's intent.
- repo root + diff as first-class input: confirmed — the `analysis_scope` block
  as a named planned delta is accurately described.
- changed seams ranked before whole-repo inventory: confirmed.
- usable non-empty partial output, skip metadata, skip remediation,
  mixed-language route context, proof_mode, oracle fields, coverage_confidence,
  limitation line, receipts, manual-candidate provenance, trust boundary,
  acceptance checklist: all confirmed accurate.

### Gap 1 — Preflight-skip structured counts

**Decision: deferred (ripr-side closure preferred).**

The cache-store skip (`lane1_repo_exposure_cache_store_skipped_large_entry`)
already populates structured `observed_seams` and `cache_limit`. The preflight
skip (`lane1_repo_exposure_large_cache_preflight_skip`) reports footprint in
prose only. The consumer needs structured `observed_seams` / `cache_limit` from
the preflight skip to render skip metadata uniformly. This is a ripr-side
closure: ripr should populate the structured fields on the preflight-skip
category. Until then, the gap stays named in both documents.

### Gap 2 — Cache persistence keying

**Decision: deferred (ripr-side closure, operational guarantee form).**

The rail's acceptance checklist says "cache persistence is explicit and
reproducible." The consumer does not need a visible contract field in the
output for cache keying — an operational guarantee (documented in SPEC-0070,
not emitted in check JSON) is sufficient. The consumer must not assert cache
keying. ripr should document the cache-keying guarantee in SPEC-0070; the gap
stays named until that documentation lands.

### Gap 3 — Per-seam `source_route`

**Decision: consumer-side closure.**

The rail's `source_route` field (e.g. `Bun.gzipSync BufferSource plus options
getter reentry`) is a downstream-computed label, not a ripr-emitted field. The
consumer (unsafe-review) synthesises it from the configured-route metadata on
`bun_cross_language_grip` and the stable-byte family classification. ripr
should annotate the SPEC-0070 gap row as "consumer-owned" and document that
consumers may compute `source_route` from grip metadata. The rail documents the
synthesis rule: `source_route` = configured-route label from
`bun_cross_language_grip` + the stable-byte family name.

### Gap 4 — Per-seam `stable_byte_family`

**Decision: consumer-side closure.**

Same as Gap 3. The `stable_byte_family` label (e.g.
`stable-byte-source-getter-reentry`) is a downstream-assigned classification,
not a ripr-emitted field. The consumer maps the configured-route metadata and
bridge inventory to the stable-byte family taxonomy (the 4 families:
`stable_byte_source_getter_reentry`, `stable_byte_source_rab_async`,
`stable_byte_source_sab_race`, `stable_byte_source_native_ffi_read`). ripr
should annotate the gap row as "consumer-owned."

### Gap 5 — Report-level diff-first mode (`mode: diff_first` / `changed_seams_first`)

**Decision: deferred (ripr-side closure, `analysis_scope` form).**

The rail's `mode: diff_first` and `changed_seams_first: true` are intended as
illustrative YAML, not stable machine-readable fields. The existing
`analysis_scope.run_status = "limited_diff_scope"` encoding satisfies the
consumer's need to distinguish diff-scoped from whole-repo output. No distinct
`mode` field needs to land in ripr. The gap stays named in SPEC-0070 to
document that `analysis_scope` is the encoding, not a literal `mode` field.
