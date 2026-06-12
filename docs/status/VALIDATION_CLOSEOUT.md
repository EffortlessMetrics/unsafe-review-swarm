# Validation closeout: fast and low-noise posture (2026-06-12)

This records what "fast and low-noise" is now *measured by* for `unsafe-review`,
the result of the first validation pass, and what remains. It is a posture
record, not a claim of proof. Every figure here is a diagnostic characterization
on a controlled fixture sample — **not** calibrated precision/recall, not a
memory-safety/UB-free/Miri-clean/site-execution claim, and not a performance SLA.
See [SUPPORT_TIERS](SUPPORT_TIERS.md) for the claim-to-proof ledger.

## What is measured now (the instruments)

The adoption + measurement substrate that landed makes leanness and noise
*measurable* rather than asserted:

- **Per-run usefulness telemetry** — `usefulness-telemetry.json`
  ([SPEC-0038](../specs/UNSAFE-REVIEW-SPEC-0038-low-noise-usefulness-telemetry.md)),
  projected from the ReviewCard/Summary/comment-plan: card inventory
  (total/actionable/new/worsened/resolved/inherited), coverage slots,
  `agent_readiness` (ready / needs_human / **requires_witness_receipt** /
  unsupported), comment selection (selected + not-selected reason and
  reason×class histograms), actionability distribution, `unfulfilled_obligation_count`,
  and a `scan_cost` section (`elapsed_ms`, `output_bytes_total`).
- **Scan diagnosability** —
  [SPEC-0035](../specs/UNSAFE-REVIEW-SPEC-0035-repo-scan-diagnosability.md):
  `elapsed_ms`, per-file timings, `output_bytes`, file counts, partial/stop_reason.
- **External resource harness** — the scheduled corpus backstop
  ([SPEC-0039](../specs/UNSAFE-REVIEW-SPEC-0039-scheduled-corpus-backstop.md))
  emits `resource-report.json` with wall time, output bytes, file/card counts,
  and peak RSS measured **externally** (`/usr/bin/time -v`; null where
  unsupported). The shipped binary stays `unsafe`-free; peak RAM is not read
  in-process (see
  [ADR-0008](../adr/UNSAFE-REVIEW-ADR-0008-resource-measurement-placement.md)).

## First validation pass (fixture sample, 2026-06-12)

Ran `first-pr` (with the telemetry above) over a representative fixture sample
spanning the noise shapes (negative controls, single-gap, multi-gap, witnessed,
agent-ready, human-review-only). Against ground truth in
`policy/calibration.toml`:

- **Low-noise — holds on the sample.** Negative-control fixtures emit 0 cards
  (no false noise on safe input); precision controls (e.g. `align_of` is not an
  alignment guard) correctly still flag; no positive fixture emitted extra
  cards. Card counts matched calibration exactly.
- **Usefulness — holds on the sample.** Each gap card carried a specific
  next-action, verify commands, and a closed-vocabulary not-selected reason for
  budget-omitted cards. Agent-readiness routing is correct *after* the
  fidelity fix below.
- **Per-run cost (trivial single-file fixtures):** wall time ~120–445 ms,
  output bundle ~35–97 KB. The dominant cost on real repos is the diff-scoped
  parse, not per-card overhead.

### Fix found and applied by the pass

The pass found a truthfulness inconsistency: a `requires_loom` card was reported
`agent_lsp_readiness = "ready"` (so the telemetry counted it immediately
agent-delegatable) while the comment-plan correctly said
`requires_witness_receipt` — two surfaces projecting from the same card
disagreeing. Fixed at the single derivation point by adding a
`RequiresWitnessReceipt` readiness state so all surfaces agree; the telemetry no
longer over-counts `ready`.

## What "fast and low-noise" is measured by

- **Fast / lean:** per-run `elapsed_ms` + `output_bytes_total` (cost in the
  bundle); external peak RSS + corpus wall-time trend on the bench.
- **Low-noise:** 0 cards on safe/quiet input; card count, actionability
  distribution, and not-selected reason×class histogram (a `lower_relevance`
  skip on a real actionable card would be visible, distinct from a correct
  FFI/miri suppression).
- **Useful:** actionable cards with next-actions; agent-readiness histogram
  (now faithful, with the receipt-gated bucket); `unfulfilled_obligation_count`
  for triage of work behind a single card.

## What remains

- **Real-PR (not just fixture) low-noise corpus** — exercise the surfaces on
  real PR diffs to characterize real-world noise, beyond the controlled sample.
- **Promote the composite Action to the source/public repo** so external repos
  can `uses: EffortlessMetrics/unsafe-review@v1` (a curated release/promotion
  step, not routine swarm work).
- **Live end-to-end Action smoke** (deferred from the action PR) and **CI
  cold-start cost** (the action installs from crates.io per run; tracked in the
  CI cost issue).
- **In-tool per-run RSS** stays parked (revivable on validated operator demand;
  ADR-0008).
