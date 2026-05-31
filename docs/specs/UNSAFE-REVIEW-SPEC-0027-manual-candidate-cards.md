# UNSAFE-REVIEW-SPEC-0027: Manual candidate cards

Status: proposed
Owner: product / cli
Created: 2026-05-31
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- TBD
Linked issues:
- #1145
Linked PRs:
- TBD
Support-tier impact: future candidate import surface
Policy impact:
- none

## Problem

Some high-quality unsafe-review findings are discovered outside the analyzer,
especially in large mixed-language repositories where full repo scans may time
out or need human route tracing. Today receipts attach evidence to existing
card identities, but there is no first-class artifact for a manually discovered
candidate that should flow through the same explain, context, witness-plan,
receipt, and outcome surfaces.

`unsafe-review` needs a ledger format for externally discovered candidates
without implying that those candidates were analyzer-discovered, witnessed,
proved, or policy-ready.

## Behavior

A manual candidate card is an advisory input artifact supplied by a reviewer,
scout lane, or external tool. It is ReviewCard-like projection input, not an
analyzer finding.

The initial file format is JSON with this top-level contract:

```text
schema_version = manual-candidate/v1
id
title
location
operation_family
unsafe_operation
invariant
safe_caller
evidence
trust_boundary
```

Required object fields:

- `location.file`: root-relative path to the candidate source location.
- `location.line`: 1-based source line when known.
- `evidence[]`: zero or more external evidence references.
- `trust_boundary`: explicit manual/advisory boundary text.

Each `evidence[]` item must include:

- `kind`: closed vocabulary such as `runtime_witness`, `model`,
  `source_trace`, `node_parity`, `human_review`, or `other`.
- `path`: local artifact path, when evidence is file-backed.
- `summary`: optional concise description of what the evidence supports.

Future import command shape:

```bash
unsafe-review candidate import target/unsafe-scout/textdecoder-candidate.json \
  --out .unsafe-review/candidates/R4R2-S001.json
```

The importer must preserve the supplied manual candidate identity. Projected
cards or card-like records must carry:

```text
source = manual
manual_candidate = true
```

Manual candidates must remain source-aware in every downstream surface. They
may project through explain, context, witness-plan, receipt, outcome, saved
JSON, SARIF, or comment-plan surfaces only when those projections preserve the
manual/advisory marker and do not treat the candidate as analyzer-discovered.

Receipts may reference manual candidate IDs, but a receipt against a manual
candidate records external evidence for that manual candidate only. It does not
prove the repository safe, prove UB, prove site execution outside the receipt,
or convert the candidate into an analyzer finding.

Outcome comparison must compare manual candidates with manual candidates by
stable manual ID and source marker. It must not report a manual candidate as an
analyzer-resolved card unless a later analyzer-discovered ReviewCard explicitly
links to the same manual ID through a reviewed linkage field.

## Example

```json
{
  "schema_version": "manual-candidate/v1",
  "id": "R4R2-S001",
  "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
  "location": {
    "file": "src/runtime/webcore/TextDecoder.rs",
    "line": 237
  },
  "operation_family": "raw_pointer_read",
  "unsafe_operation": "core::slice::from_raw_parts",
  "invariant": "&[u8] memory must not be concurrently mutated",
  "safe_caller": "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))",
  "evidence": [
    {
      "kind": "runtime_witness",
      "path": "target/unsafe-scout/textdecoder-shared-race-route.out"
    },
    {
      "kind": "model",
      "path": "target/unsafe-scout/miri-textdecoder-shared-slice.out"
    }
  ],
  "trust_boundary": "manual candidate; not analyzer-discovered; not proof of repository safety"
}
```

## Projection Contract

Manual candidate projections must reuse existing ReviewCard vocabulary where it
fits, including operation family, location, next action, witness route, missing
evidence, and trust-boundary fields. They must not create another classification
truth or silently drop fields that identify the candidate as manual.

If a manual candidate cannot be projected faithfully into a surface, that
surface must reject or omit it with an explicit reason instead of degrading it
into an analyzer ReviewCard.

## Non-goals

- no analyzer heuristic for discovering these candidates
- no receipt-only workaround that lacks a card-like manual artifact
- no witness execution
- no automatic comments
- no source edits
- no default blocking policy
- no claim of proof, UB-free status, Miri-clean status, site execution,
  calibrated precision/recall, or policy readiness
- no claim that imported candidates are analyzer-discovered

## Required Evidence

- schema parser tests for valid and invalid `manual-candidate/v1` JSON
- CLI import e2e coverage for `candidate import`
- projection tests proving `source = manual` and `manual_candidate = true` are
  preserved in explain, context, witness-plan, saved JSON, and outcome surfaces
- receipt tests for manual candidate IDs
- negative tests proving manual candidates are not labeled analyzer-discovered

## Acceptance Examples

- Importing a valid manual candidate JSON writes a canonical candidate artifact
  with the same ID, source marker, location, operation family, unsafe operation,
  invariant, safe caller, evidence references, and trust boundary.
- `explain` and `context` for a manual candidate state that it is manual and
  advisory, and they include the external evidence packet without claiming that
  unsafe-review found the issue.
- `witness-plan` routes manual evidence as suggested follow-up work without
  executing witnesses.
- A receipt against a manual candidate ID can be imported or audited only as
  evidence for that manual candidate.
- Outcome comparison preserves manual source markers and compares manual IDs
  deterministically across snapshots.

## CI Proof

Current contract-only proof:

```bash
cargo run --locked -p xtask -- check-docs
cargo run --locked -p xtask -- check-doc-artifacts
cargo run --locked -p xtask -- check-spec-status
cargo run --locked -p xtask -- check-pr
cargo run --locked -p xtask -- source-divergence
git diff --check
```

Future runtime proof must add focused CLI and projection tests when
`candidate import` lands.

## Metrics / Promotion Rule

Remain proposed until the importer and at least explain, context, witness-plan,
receipt, and outcome projections preserve the manual candidate source marker.

## Failure Modes

- imported candidates appear indistinguishable from analyzer findings
- receipts become the only durable artifact for manual discoveries
- projections drop external evidence references or manual trust-boundary text
- outcome comparison conflates manual and analyzer-discovered cards
