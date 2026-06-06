# UNSAFE-REVIEW-SPEC-0033: LLM context packet

Status: proposed
Owner: product / core
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
Support-tier impact: stabilizes the agent-facing optic; no new analyzer tier
Policy impact:
- none

## Problem

`unsafe-review` is the **unsafe coverage instrument** (SPEC-0028): it answers
"is this seam reviewable?". Surface 4 in that umbrella is authoring-time feedback
whose consumer is **a coding agent editing a file**, not a human in VS Code. The
machinery already exists: a saved `lsp.json` projection and a live
`unsafe-review lsp` server both expose hover, code actions, and five read-only
commands (SPEC-0012, SPEC-0018), and `unsafe-review context <card-id> --json`
emits a bounded agent packet — obligations, missing evidence, allowed repairs,
confirmation cue, do-not-do (SPEC-0013).

The gap is architectural, not analytic: every shape is **human-shaped**. Hover is
keyed to a cursor; actions are clicked; `context` is keyed to a card id the human
already picked. An agent editing `file.rs` lines Y-Z has no card id. It must
scrape `lsp.json`, map diagnostics back to lines itself, then call `context`
once per matching card — N round trips, with no signal that the diagnostics it
scraped are still current, and no way to ask only about the lines it changed.
This spec makes the existing optic answer one query: **obligations + cues +
repairs for `file:line` in one call**. It introduces no detector.

## Behavior

This is a projection of the per-card coverage block defined in SPEC-0029. It
re-shapes how that block is queried; it does not redefine coverage or add truth.

### A stable LLM-facing packet

`unsafe-review context --json` and the saved `lsp.json` payload become the
**canonical agent interface**. The per-card packet carries, for each affected
card:

```text
coverage_slots        the SPEC-0029 coverage block (contract / guard / test-reach / witness-receipt)
missing_evidence      what evidence is absent or weak, per obligation
allowed_repairs       bounded, card-scoped repairs the agent may perform
do_not_do             the forbidden-action list (travels in every packet)
receipt_witness_route witness route + verify command from the card (copy-only)
baseline_state        new / worsened / resolved / inherited vs baseline (SPEC-0030)
comment_plan_status   whether this card is anchored in the comment plan (SPEC-0032)
staleness_marker      a refresh generation id so the agent knows diagnostics are current vs stale
```

The `staleness_marker` is the one new field of substance: a monotonic
`refresh_generation` id (and the analyzed `base`) stamped on every packet, so an
agent comparing two reads can tell whether the file changed under it. It is a
freshness signal, never a freshness guarantee. The packet keeps the SPEC-0013
fields (`mode = bounded_repair_packet`, `source = review_card`,
`policy = advisory`, card identity) unchanged.

### A query path keyed to a file:line range

An agent asks "obligations + allowed repairs for file X lines Y-Z" and gets a
**bounded list of packets in one call**, instead of scrape-then-N-calls:

```bash
unsafe-review context --file <path> --lines <Y>-<Z> --json
```

The result is a `mode = file_range_scan` envelope wrapping zero or more per-card
packets whose unsafe site overlaps the requested range, plus the
`staleness_marker`. The list is bounded (honoring the existing `maxCards`-style
cap) and ordered deterministically by site line. An empty list means no
reviewable seam overlaps those lines — never "these lines are safe".

### Changed-line filter

A PR agent passes the diff so it sees only diff-touched obligations:

```bash
unsafe-review context --file <path> --lines <Y>-<Z> --changed-only --json
```

`--changed-only` restricts the returned packets to cards whose site falls on
lines added or modified versus the analyzed base, reusing the SPEC-0030 baseline
movement rather than recomputing a diff. This keeps an agent's attention on what
the PR introduced, the same scoping the gate surface already applies.

## Non-goals

- no execution of witnesses, tests, Miri, sanitizers, or any tool
- no source edits, patch generation, or SAFETY-comment insertion
- no comment posting and no blocking
- no proof, UB-free, Miri-clean, site-execution, or calibrated precision/recall
  claim; an empty packet list is never a safety claim
- no new analyzer truth, detector, or reclassification — coverage stays as
  measured in SPEC-0029
- no agent runtime; `unsafe-review` does not drive the model, it answers queries

## Trust boundary

The packet is **advisory coverage evidence**: static unsafe-contract review, not
memory-safety proof, not UB-free status, not Miri-clean status, and not a
site-execution claim unless a matching witness receipt says so. It is a
**bounded optic for agents, not authority**: copy-only, no execution, no writes.
The `do_not_do` list and the trust-boundary string travel inside every packet
and every `file_range_scan` envelope, so an agent that reads only the JSON still
carries the boundary. Widening who can reach the coverage evidence does not widen
what the evidence claims. Cross-tool contracts with the orchestrator and siblings
are governed by SPEC-0028 and [`docs/interop/sibling-tools.md`](../interop/sibling-tools.md).

## Proof obligations

```bash
cargo run --locked -p xtask -- check-spec-status
cargo run --locked -p xtask -- check-docs
cargo run --locked -p xtask -- check-pr
cargo run --locked -p xtask -- check-calibration
cargo run --locked -p xtask -- check-manual-candidate-examples
cargo run --locked -p xtask -- check-dogfood
```

## Machine check

Registered in `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` and routed from
`docs/specs/UNSAFE-REVIEW-SPEC-START-HERE.md`; validated by
`cargo run --locked -p xtask -- check-spec-status` and
`cargo run --locked -p xtask -- check-docs`.
