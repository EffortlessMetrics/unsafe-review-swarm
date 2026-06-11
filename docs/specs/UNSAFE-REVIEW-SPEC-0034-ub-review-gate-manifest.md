# UNSAFE-REVIEW-SPEC-0034: ub-review gate manifest (unsafe-review-gate.json)

Status: proposed
Owner: product / cli
Created: 2026-06-06
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- plans/0.2.0/implementation-plan.md
Linked issues:
- none
Linked PRs:
- TBD
Support-tier impact: ub-review integration surface (meta-orchestration projection)
Policy impact:
- none

## Problem

`ub-review` already runs `unsafe-review first-pr --root --base` as one of its
sensor lanes, but it captures only stdout/stderr and discards the structured
artifacts the run produced — `cards.json`, `comment-plan.json`,
`witness-plan.md`, `review-kit.json`. Its model lanes then consume flat text.
The instrument's richest output (the per-card coverage block of
UNSAFE-REVIEW-SPEC-0029, the baseline movement of UNSAFE-REVIEW-SPEC-0030) never
reaches the orchestrator except as prose to scrape.

The sibling sensors already solved this with routable manifests: `ripr` ships
`gate-decision.json` (`schema_version` 0.1, `status`, `decisions[]`) plus
`baseline-debt-delta.json`, and ripr-swarm #1038 adds a canonical
`new_unsuppressed` movement counter; `cargo-allow` emits receipts carrying a
`schema_version`. unsafe-review-swarm #1522 asks this manifest to be co-designed
with ripr's envelope so the orchestrator routes every sensor the same way.
unsafe-review must emit one stable manifest that points at its structured
artifacts and summarizes movement — so `ub-review` routes by schema, never by
markdown.

This is a projection and contract surface (UNSAFE-REVIEW-SPEC-0028, surface 5).
It introduces no analyzer behavior.

## Behavior

### One stable manifest

A `first-pr`/`repo` run emits exactly one `unsafe-review-gate.json` alongside the
existing artifacts. It is a thin index over already-emitted files plus the
movement summary; it recomputes nothing.

```json
{
  "schema_version": "unsafe-review-gate/v1",
  "dialect": "unsafe-review",
  "status": "advisory",
  "summary": { "new_gaps": 2, "worsened_gaps": 1, "resolved_gaps": 3, "inherited_gaps": 91 },
  "artifacts": {
    "cards": "cards.json",
    "comment_plan": "comment-plan.json",
    "repair_queue": "repair-queue.json",
    "receipt_audit": "receipt-audit.json",
    "review_kit": "review-kit.json",
    "pr_summary": "pr-summary.md",
    "sarif": "cards.sarif",
    "lsp": "lsp.json",
    "policy_report": "policy-report.json"
  },
  "trust_boundary": "static unsafe-review coverage evidence; not proof, not a merge verdict",
  "tool": "unsafe-review",
  "tool_version": "<semver>"
}
```

- `summary` is the four-bucket movement block of UNSAFE-REVIEW-SPEC-0030, copied
  verbatim, not re-derived. `status` is always `advisory` here; the manifest
  carries posture, never a block verdict.
- `artifacts` are relative pointers to the structured files the run already
  wrote (UNSAFE-REVIEW-SPEC-0029 projections). Missing optional artifacts are
  omitted, not faked.
- `trust_boundary` is a fixed advisory string; it makes no proof / UB-free /
  Miri-clean / site-execution / calibrated-precision-recall / policy-readiness
  claim.

### Envelope co-designed with ripr (route by schema)

The envelope is co-designed with ripr's `gate-decision.json` so `ub-review`
routes both sensors by schema rather than by tool-specific parsing (#1522, ripr
#1038/#1041):

- `schema_version` form (integer vs string) is agreed once across the sibling
  manifests rather than chosen unilaterally (cargo-allow #1465); whichever form
  the family adopts, this manifest matches it.
- a `dialect` marker (cargo-allow #1470) names the emitting sensor so a shared
  router can dispatch a heterogeneous set of sensor manifests.
- the movement summary key names align with ripr's canonical movement counter
  (`new_unsuppressed`, ripr-swarm #1038) so the orchestrator reads one movement
  vocabulary across sensors.

The agreed shapes live in
[`docs/interop/sibling-tools.md`](../interop/sibling-tools.md); this spec must
not fork a parallel format.

### The manifest is the consumer contract

`ub-review` (and its model lanes) read state from this manifest only. They must
not infer status, movement, or comment selection from `witness-plan.md` or any
human-rendered surface. "Easy" for this surface means no markdown scraping: one
file, one schema, one route. The human and markdown surfaces remain unchanged
and advisory; they are not the orchestrator's input.

## Non-goals

This spec does not:

- decide blocking or emit a merge verdict — `status` is advisory posture; the
  orchestrator owns the gate decision (UNSAFE-REVIEW-SPEC-0028 boundary),
- post comments, run witnesses, run other sensors (ripr, cargo-allow, clippy),
  or edit source,
- redefine the coverage slots (UNSAFE-REVIEW-SPEC-0029) or the movement buckets
  (UNSAFE-REVIEW-SPEC-0030) — it points at them,
- widen analyzer detection or add hazard families,
- make any proof, UB-free, Miri-clean, site-execution, calibrated
  precision/recall, or policy-readiness claim.

## Trust boundary

`unsafe-review-gate.json` is a routing manifest over advisory coverage
artifacts. It is not a merge verdict and not proof. Its `summary` reports
coverage movement, not safety; its `artifacts` point at static unsafe-review
evidence; its `status` is advisory and never blocks. The actor that blocks a
merge or posts a comment is `ub-review`, never the manifest.

## Proof obligations

- `cargo test -p unsafe-review-core` — manifest serialization; movement summary
  copied verbatim from the SPEC-0030 block; artifact pointers match emitted
  files; fixed advisory `trust_boundary` and `status`.
- `cargo test -p unsafe-review` — `first-pr`/`repo` emits exactly one
  `unsafe-review-gate.json` whose pointers resolve to the run's artifacts.
- schema-alignment fixture checked in `check-pr` — `schema_version` form and
  `dialect` marker match the shapes recorded in `docs/interop/sibling-tools.md`.
- `cargo run --locked -p xtask -- check-pr`.

## Machine check

Registered in `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` and routed from
`docs/specs/UNSAFE-REVIEW-SPEC-START-HERE.md`; validated by
`cargo run --locked -p xtask -- check-spec-status` and
`cargo run --locked -p xtask -- check-docs`.
