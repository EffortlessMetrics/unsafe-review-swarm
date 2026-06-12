# UNSAFE-REVIEW-ADR-0008: Resource measurement — per-run point values in-tool, distributions on the bench

Status: proposed
Date: 2026-06-12
Owner: core / repo-infra
Linked specs:
- ../specs/UNSAFE-REVIEW-SPEC-0035-repo-scan-diagnosability.md
Linked docs:
- ../contributing/dependency-pr-policy.md
- ../contributing/AGENT-ORCHESTRATION.md (sections 11-12)
Linked PRs:
- TBD (output-byte footprint #1618; RSS telemetry: TBD)

## Decision

`unsafe-review` reports its own per-run resource cost as **point values in the run telemetry**: CPU/time (`elapsed_ms`, `file_timings`), disk (`output_bytes`), and memory (`current_rss_bytes`, `peak_rss_bytes`). Every resource field is **optional, additive, `None` on unsupported platforms, and diagnostic only** — not a coverage, proof, safety, UB-free, Miri-clean, site-execution, or performance claim.

Memory is read with a **small, contracted, ledgered FFI call** where no safe path exists, and via a **safe path where one exists** (e.g. Linux current RSS through `/proc`). No new *shipped* dependency is added for it.

Resource **distributions** (mean / p50 / p95 / p99, derived from sampling over time) are **not** in the per-run telemetry. They belong to an opt-in profiling / bench surface, off the critical path, and bounded.

## Context

The product goal is fast **and** lean (disk/RAM/CPU) **and** clean. To make leanness *measurable* rather than asserted, the tool reports its own resource cost. The open questions were *where* each metric lives and *how* memory is read on a stable-Rust, minimal-dependency, `unsafe_code`-by-default-forbidden codebase.

## Rationale

- **A metric's home is its purpose.** Per-run point values answer the operator's "what did this scan cost / will it OOM / how do I size CI" question, so they live in the run telemetry. Distributions/percentiles answer a profiler's or budget-owner's *tail* question on a cadence, so they live on a bench — in per-PR output they would be noise (the low-noise rule: a metric with no consumer for that surface is noise).
- **The instrument must not perturb what it measures.** Measuring leanness must not add shipped footprint, and measuring at all must stay bounded. A contracted ledgered FFI call ships nothing; a runtime crate would ship footprint to measure leanness (self-defeating). Distributions need a sampler, which perturbs and can grow unbounded — hence bench-only and bounded (reservoir sampling).
- **Cleanest tool for *this* job, not a blanket rule.** For an in-product reading with no safe wrapper, a small ledgered `unsafe` FFI is leaner than a shipped dependency; where a safe path exists, use it (no unsafe). The `unsafe` is governed — a `#[allow(unsafe_code, reason = …)]` / cargo-allow ledger entry with a real `# Safety` contract — which is the tool meeting the exact standard it asks of others.

## Consequences

Positive:

- operators see per-run scan cost (time, disk, current+peak RSS); `current` is poll-able as a building block for an external memory log;
- leanness is regression-trackable;
- no new shipped dependency;
- any `unsafe` is bounded, contracted, and governed.

Negative / limits:

- RSS is process-wide and platform-dependent — a diagnostic aperture, not a precise analysis-memory figure;
- some platforms may report a resource field as `None` (truthful absence over forced complexity);
- memory distributions/percentiles are deferred to a future bench surface.

## Alternatives considered

### Runtime dependency (e.g. `memory-stats`, `sysinfo`) for RSS

Rejected. It ships footprint in order to *measure* leanness, which is self-defeating; a contracted ledgered FFI call ships nothing and is leaner.

### Test-harness-only RAM (no in-product field)

Rejected. Per-run RAM is genuine operator signal (scan cost, CI-runner sizing, OOM-avoidance), not merely a regression metric. (Earlier reasoning over-weighted leanness/credibility and mis-classified per-run RAM as noise; corrected.)

### Per-run distributions / percentiles in the telemetry

Deferred. Percentiles require a sampled timeseries (a sampler perturbs and is unbounded). They belong on an opt-in, bounded bench surface, not the per-PR critical path. CPU per-file percentiles are nearly free (derived from the already-captured `file_timings`) and would be the first such metric if/when that lane opens.
