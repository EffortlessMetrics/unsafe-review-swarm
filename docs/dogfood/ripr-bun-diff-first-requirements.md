# ripr Bun Diff-First Requirements

Status: future tooling-interface requirements

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

A `ripr` receipt must not become:

- witness execution;
- Miri-clean evidence;
- site-execution proof;
- proof of UB;
- proof of memory safety;
- a calibrated precision/recall claim;
- a default blocking policy.

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
