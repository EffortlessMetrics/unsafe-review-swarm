# UNSAFE-REVIEW-ADR-0008: Resource measurement — time/disk in-tool, peak RAM and distributions on the bench

Status: proposed (decision revised 2026-06-12 to external-first — see Revision)
Date: 2026-06-12
Owner: core / repo-infra
Linked specs:
- ../specs/UNSAFE-REVIEW-SPEC-0035-repo-scan-diagnosability.md
Linked docs:
- ../contributing/dependency-pr-policy.md
- ../contributing/AGENT-ORCHESTRATION.md (sections 11-12)
Linked PRs:
- #1615 (per-file timing, in-tool); #1618 (output-byte footprint, in-tool)
- #1620 (in-tool current+peak RSS via ledgered FFI — implemented and green, but **parked**; see Revision)
- external peak RAM harness: SPEC-0039 (forthcoming)

## Decision

`unsafe-review` reports the resource cost that is a **free byproduct of a normal run** as point values in the run telemetry: CPU/time (`elapsed_ms`, `file_timings`) and disk (`output_bytes`). These need no syscall, no `unsafe`, and no new dependency — they fall out of work the tool already does.

**Memory (peak/current RSS) and resource distributions** (mean / p50 / p95 / p99) are measured **externally, on a scheduled nightly/bench harness** (SPEC-0039), using the platform's own facility (e.g. `/usr/bin/time -v` on Linux; `None` with an explicit reason where unsupported). They are **not** read inside the shipped binary.

The shipped binary therefore stays at `unsafe_code = forbid` — **no `unsafe` in the product** for the sake of a diagnostic field. Every resource value, in-tool or external, is **optional, additive, `None` on unsupported platforms, and diagnostic only** — not a coverage, proof, safety, UB-free, Miri-clean, site-execution, or performance claim, and never an SLA.

## Context

The product goal is fast **and** lean (disk/RAM/CPU) **and** clean. To make leanness *measurable* rather than asserted, the tool's cost is reported. The open questions were *where* each metric lives and *how* memory is read on a stable-Rust, minimal-dependency, `unsafe_code`-by-default-forbidden codebase.

## Rationale

- **A metric's home is its purpose.** Time and disk are intrinsic to every run and answer the operator's "what did this scan cost / how do I size CI" question, so they live in the per-run telemetry for free. Peak RAM and distributions answer a *leanness-regression* / *budget-owner* question on a cadence — that is a bench question, so it lives on the scheduled harness. In per-PR output, distributions would be noise (the low-noise rule: a metric with no consumer for that surface is noise).
- **The instrument must not perturb what it measures.** Measuring leanness must not add shipped footprint *or* relax a shipped safety invariant. Reading peak RAM in-process requires a syscall (`getrusage`/`GetProcessMemoryInfo`) reachable only through `unsafe` FFI, which would relax the workspace `unsafe_code = forbid` for one diagnostic field. Measuring it **externally** touches the binary not at all — the leanest possible instrument.
- **Cleanest tool for *this* job, not a blanket rule.** For an *unvalidated* in-product RAM reading, the cleanest tool is external measurement (zero binary change), not a governed `unsafe` block. `unsafe` remains available as a governed exception (`#[allow(unsafe_code, reason = …)]` + ledger + `# Safety` contract) for a job that genuinely needs it in-product — this one does not, yet.

## Consequences

Positive:

- time + disk cost are visible per run with zero added cost or risk;
- peak RAM and trend are tracked on the bench, off the PR critical path;
- the shipped binary stays `unsafe`-free (`unsafe_code = forbid`), the strongest invariant for a tool that reviews unsafe code;
- no new shipped dependency.

Negative / limits:

- operators do not get per-run peak RAM *inside the tool's own output* — they must read it from the bench report or wrap the binary externally (e.g. `/usr/bin/time`). This is the deferred cost, revisitable on demand (see Revision);
- external RSS is process-wide and platform-dependent — a diagnostic aperture, not a precise analysis-memory figure;
- some platforms report a resource field as `None` (truthful absence over forced complexity).

## Alternatives considered

### In-tool per-run RSS via a contracted, ledgered `unsafe` FFI

**Deferred (implemented, parked).** PR #1620 built this cleanly — current+peak RSS via a governed `unsafe` FFI (`getrusage`/`GetProcessMemoryInfo`), no new dependency, full `# Safety` contracts, all gates green — relaxing the workspace lint `forbid` → `deny`. It is parked as a draft because per-run in-tool RAM is an **unvalidated operator use case**, and shipping `unsafe` (and relaxing a workspace-wide invariant) ahead of validated demand contradicts the "measurement and validation, not speculative surface" thesis. **Revival trigger:** a real operator needs per-run current+peak RSS in their own pipeline's status JSON; the #1620 branch is the ready, reviewed snapshot.

### Runtime dependency (e.g. `memory-stats`, `sysinfo`) for RSS

Rejected. It ships footprint in order to *measure* leanness, which is self-defeating; external measurement ships nothing.

### Per-run distributions / percentiles in the telemetry

Rejected for the per-PR path. Percentiles require a sampled timeseries (a sampler perturbs and is unbounded). They belong on the scheduled bench harness (SPEC-0039), bounded. CPU per-file percentiles are nearly free (derived from the already-captured `file_timings`) and would be the first such bench metric.

## Revision (2026-06-12): external-first

This ADR originally decided **in-tool** per-run RSS via a ledgered `unsafe` FFI (point values in the run telemetry). That decision is **reversed to external-first** before it shipped:

- PR #1620 implemented the in-tool approach green, but was **parked** for lack of validated operator demand;
- keeping the shipped binary at `unsafe_code = forbid` is preferred until a real operator needs per-run RAM in-tool;
- peak RAM + distributions now live on the scheduled bench harness (SPEC-0039).

The durable principles above (metric-home-matches-purpose, instrument-must-not-perturb, cleanest-tool-for-the-job) are unchanged — applying them with the parked-PR evidence in hand now points to external measurement. A future PR that wants in-tool RSS must cite and refute this revision (and revive #1620), not assume the option was never considered.
