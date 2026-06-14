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

## Corpus validation: inherited debt and resolved evidence (2026-06-12)

Beyond the per-fixture sample, two corpus cases pin the adoption-critical
movement shapes, and a rollup aggregates the usefulness signal across a
representative subset:

- **Corpus rollup** (`xtask corpus-usefulness` →
  `corpus-usefulness-rollup.json`, documented in
  [SPEC-0039](../specs/UNSAFE-REVIEW-SPEC-0039-scheduled-corpus-backstop.md)):
  builds the binary once and runs `first-pr` over a curated subset, aggregating
  the SPEC-0038 telemetry (card inventory, agent-readiness, coverage slots,
  not-selected reason×class, scan-cost range). Off the PR critical path; a
  committed schema-only sample is gate-checked. On the current subset the cards
  were all actionable with no unexpected noise — a *subset* signal, not a
  precision claim.
- **Brownfield / inherited debt** (`raw_pointer_deref_brownfield_inherited`):
  a repo with a baselined pre-existing unsafe gap where a safe-only PR shows
  `new_gaps=0, worsened_gaps=0, inherited_gaps=1`, card class `baseline_known` /
  `inherited`, `comment_plan_status=not_eligible`, `selected_count=0`, and
  `no-new-debt` exits 0. Proves inherited unsafe debt is **visible but not PR
  comment noise** — the property mature repos need to adopt the tool.
- **Resolved evidence** (`raw_pointer_deref_resolved`): a PR that adds a
  `# Safety` contract to a *retained* `pub unsafe fn` shows resolved movement
  (`resolved_gaps=1, new_gaps=0`) — an evidence improvement, not a deletion,
  registering as resolved.

### Finding: in-scope evidence reclassifies; "resolved" comes from scope-exit

A precise reading of the resolved case surfaced a behavior nuance worth
recording. Adding a `# Safety` contract to an unsafe fn that stays **in diff
scope** does *not* resolve its card — it **reclassifies** it from
`contract_missing` to `guarded_unwitnessed` (calibration:
`public_unsafe_fn_with_safety_docs` is a `guarded_unwitnessed` card — still
actionable, now needing a witness). The card persists as a less-severe class.
"Resolved" movement arises when the unsafe **site leaves diff scope**: removed,
or (as in the resolved fixture) a doc-only change that does not touch the unsafe
body, so the site falls out of the changed hunk and the baselined card goes
unmatched.

So the honest framing of "unsafe-review rewards evidence improvement" is: for an
in-scope site, evidence is rewarded by **reclassification to a less-severe
card** (contract_missing → guarded_unwitnessed → guarded_and_witnessed), not by
resolution; full resolution of an in-scope site requires discharging every
obligation. This is defensible — an unwitnessed-but-contracted unsafe site is
still worth an advisory card — but it means "resolved" is a narrower signal than
it first appears. Whether a fully-evidenced in-scope site *should* resolve (vs.
settle at `guarded_and_witnessed`) is a product question for a future lane, not
a defect.

## What remains

- **Real external-repo PR noise reading** — the rollup and corpus cases above
  use local fixtures; exercising the surfaces on real external PR diffs (the
  dogfood targets need network seeding) remains, to characterize real-world
  noise beyond controlled fixtures.
- **Promote the composite Action to the source/public repo** so external repos
  can `uses: EffortlessMetrics/unsafe-review@v1` (a curated release/promotion
  step, not routine swarm work).
- **Live end-to-end Action smoke** (deferred from the action PR) and **CI
  cold-start cost** (the action installs from crates.io per run; tracked in the
  CI cost issue).
- **In-tool per-run RSS** stays parked (revivable on validated operator demand;
  ADR-0008).

## Next lane (evidence-backed)

The measured low-noise validation layer is complete: inherited debt is visible
but quiet, evidence improvements register (as reclassification for an in-scope
site, as resolved movement on scope-exit), and per-run plus corpus cost are
instrumented. The next lane should be chosen from observed friction, not roadmap
inertia. Candidates this lane surfaced:

- the resolve-vs-reclassify product question above (should a fully-evidenced
  in-scope site resolve, or settle at `guarded_and_witnessed`?);
- a real external-repo PR noise reading (needs dogfood-target seeding);
- the deferred items in *What remains*.

Everything here is a characterization on a controlled corpus — explicitly
**not** a zero-false-positive, calibrated precision/recall, or safety-proof
claim.

## Real validation beats synthetic; run per release

Fixtures pass by construction and `check-pr` is a dev-tree gate only. The
**live Action smoke** (installing the published binary from crates.io and
running it against a real diff) and the **real-crate dogfood run** are what
surface framing issues, selection gaps, and version-skew problems that
controlled fixtures cannot reach. Both should be run each release cycle before
promotion to the public repo.

## Card-correctness session learnings (2026-06-14)

A 28-card correctness session (#1672–#1707) over the fixture suite and fresh-crate
dogfood surfaced a set of systemic lessons that are encoded in the doc system.
Key facts for future validation passes:

**Green CI can prove the wrong property.** At the start of the session, 600+
fixture tests were green. The fixture suite was blind to the largest class of
false positive — detectors firing in safe-context code — because every wave-1
fixture placed the operation inside `unsafe { }`. Green CI proved the fixtures
passed; it did not prove the detectors were correct on real code. The capstone
dogfood run (#1700, five fresh crates: `smallvec`, `arrayvec`, `memchr`,
`hashbrown`, `bytes`) confirmed that all identified FP classes were gone on
unseen code after the mechanism-level fixes.

**The bug class is the syntax-first discipline tax on the fallback text path.**
unsafe-review is syntax-first and build-free: it analyzes syntax, tokens, and diff
hunks without performing target-repo type or trait resolution at scan time. That is
a feature (fast, portable, works on incomplete PRs), but it carries a discipline
tax — the fallback text/substring detection path must explicitly re-derive every
structural property the `ra_ap_syntax` AST path encodes for free (scope,
call-vs-definition, binding identity, span type). Almost all false positives in the
session traced to the text path missing one of five disciplines (scope gate,
definition-vs-call, same-origin discharge, string/comment masking, path-segment
anchoring). These disciplines are now encoded in the SPEC-0005 appendix as D1–D5.

**Collapse-to-root: ~30 instances -> ~5 root fixes.** Rather than patching 28+
instances individually, a white-box audit identified the five mechanism classes
above. Fixing the mechanisms at the dispatch/evidence level cleared all instances
simultaneously and left fixture-encoded negative controls for regression
prevention.

**Fresh-crate dogfood is the required pre-release step.** The fixture suite is an
author-assumption encoder. Fresh-crate runs are the independent adversarial check.
Treat them as required validation, not optional signal, before each release
promotion.

## Release cadence: done is not delivered

Validated work should ship promptly. Do not let the staged-but-unshipped pile grow
until the staging cost (rebase, re-proof, drift from main) exceeds the benefit of
further batching. Validation increases trust but also raises the release bar —
each additional proof round makes the release surface larger and harder to hold.
The operative principle: **release before hardening the next rail.**

This is a cadence principle, not a release decision. It is advisory: the specific
release timing is an owner call. The anti-pattern it guards against is "let it
bake one more session" as a default, applied reflexively rather than because a
specific known gap warrants the delay.

