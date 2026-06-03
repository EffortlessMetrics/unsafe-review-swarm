# UNSAFE-REVIEW-SPEC-0027: Manual candidate cards

Status: proposed, partial-runtime
Owner: product / cli
Created: 2026-05-31
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- plans/0.2.0/implementation-plan.md
Linked issues:
- #1145
Linked PRs:
- TBD
Support-tier impact: candidate import surface
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
fix_options[]
test_targets[]
do_not_touch[]
```

Field rules:

- `location.file`: root-relative path to the candidate source location.
- `location.line`: 1-based source line when known.
- `evidence[]`: zero or more external evidence references.
- `trust_boundary`: explicit manual/advisory boundary text.
- `fix_options[]`: optional copy-only implementer guidance for candidate-local
  repair approaches.
- `test_targets[]`: optional copy-only test or witness targets that should
  validate the candidate-local change.
- `do_not_touch[]`: optional copy-only non-goals that must stay out of the
  candidate-local change.

Each `evidence[]` item must include:

- `kind`: closed vocabulary such as `runtime_witness`, `model`,
  `source_trace`, `node_parity`, `human_review`, or `other`.
- `path`: local artifact path, when evidence is file-backed.
- `summary`: optional concise description of what the evidence supports.
- `command`: optional exact external command that produced the evidence.
- `limitation`: optional concise statement of what the evidence does not prove.

Import command shape:

```bash
unsafe-review candidate import target/unsafe-scout/textdecoder-candidate.json \
  --out .unsafe-review/candidates/R4R2-S001.json
unsafe-review candidate list --format json
```

The repository keeps committed examples under `docs/examples/manual-candidates/`
so release and dogfood smokes can exercise import without depending on an
external scout artifact.

The importer must preserve the supplied manual candidate identity. Projected
cards or card-like records must carry:

```text
source = manual
manual_candidate = true
analyzer_discovered = false
```

Manual candidates must remain source-aware in every downstream surface. They
may project through explain, context, witness-plan, receipt, outcome, saved
JSON, SARIF, or comment-plan surfaces only when those projections preserve the
manual/advisory marker and do not treat the candidate as analyzer-discovered.

Receipts may reference manual candidate IDs, but a receipt against a manual
candidate records external evidence for that manual candidate only. It does not
prove the repository safe, prove UB, prove site execution outside the receipt,
or convert the candidate into an analyzer finding.
Receipt audit output for a matched manual candidate must preserve the manual
marker and include copy-only route, invariant, external evidence command/
limitation, fix option, test target, do-not-touch, next-action, and trust-boundary
cues so a reviewer can continue the manual candidate handoff after attaching a
receipt.

Outcome comparison must compare both single `manual-candidate/v1` artifacts and
aggregate `manual-candidates/v1` indexes with manual candidates by stable manual
ID and source marker. It must not report a manual candidate as an
analyzer-resolved card unless a later analyzer-discovered ReviewCard explicitly
links to the same manual ID through a reviewed linkage field.

## Example

```json
{
  "schema_version": "manual-candidate/v1",
  "id": "R4R2-S001",
  "source": "manual",
  "manual_candidate": true,
  "analyzer_discovered": false,
  "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
  "location": {
    "file": "src/runtime/webcore/TextDecoder.rs",
    "line": 237
  },
  "operation_family": "raw_pointer_read",
  "unsafe_operation": "core::slice::from_raw_parts",
  "invariant": "&[u8] memory must not be concurrently mutated",
  "safe_caller": "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))",
  "fix_options": [
    "Copy SharedArrayBuffer-backed bytes into stable owned storage before creating a Rust slice"
  ],
  "test_targets": [
    "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
  ],
  "do_not_touch": [
    "Do not rewrite unrelated TextDecoder encoding paths"
  ],
  "evidence": [
    {
      "kind": "runtime_witness",
      "path": "target/unsafe-scout/textdecoder-shared-race-route.out",
      "summary": "Bun TextDecoder route reaches shared backing bytes through safe JS",
      "command": "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts",
      "limitation": "runtime route evidence only; not memory-safety proof and not analyzer-discovered"
    },
    {
      "kind": "model",
      "path": "target/unsafe-scout/miri-textdecoder-shared-slice.out",
      "summary": "Miri model covers shared-slice aliasing shape outside Bun runtime",
      "command": "cargo +nightly miri test textdecoder_shared_slice_model",
      "limitation": "model evidence only; does not prove the Bun site executed under Miri"
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

Manual candidate context and witness-plan projections may include a derived
implementer handoff. That handoff must come from the imported candidate fields,
including file:line, safe caller route, unsafe operation, operation family,
invariant, external evidence references, evidence commands and limitations,
candidate-specific fix options, test targets, non-goals, and stop condition. It must
remain copy-only and must not mark the candidate analyzer-discovered, run
witnesses, edit source, or broaden the task to unrelated unsafe sites.

Manual candidate list/reporting projections must load only
`.unsafe-review/candidates/*.json` artifacts, preserve sorted manual IDs,
include `source = manual`, `manual_candidate = true`, and
`analyzer_discovered = false`, summarize the imported candidate mix with
advisory `operation_families` and `evidence_kinds` count maps, include
copy-only implementer handoff cues for the file:line target, safe caller route,
invariant, evidence packet, non-goals, candidate-specific fix options, test
targets, and stop line when available, and repeat the ReviewCard-only artifact
relationship plus a structured `reviewcard_artifact_applicability` map. The
map must mark `cards.json`, SARIF, comment-plan, saved LSP, and repair-queue
surfaces with `decision = reviewcard_only`, and mark policy-report with
`decision = reviewcard_only_follow_up` until that rail is explicitly accepted
or rejected. All entries must keep `applies_to_manual_candidates = false` and
`manual_candidate_markers_allowed = false`. They must not add manual candidates
to `cards.json`, SARIF, comment-plan, saved LSP, repair-queue, or policy-report
surfaces.
The first-pr artifact verifier must reject manual-candidate marker leakage
(`source = manual`, `manual_candidate`, or `analyzer_discovered`) in the
ReviewCard-only first-pr artifacts: `cards.json`, `cards.sarif`,
`comment-plan.json`, `lsp.json`, and `repair-queue.json`.

`review-kit.json` may include a bounded `candidate_queue` under the manual
candidate handoff so reviewers and agents can see more than the first imported
candidate. That queue must stay copy-only, preserve sorted manual IDs and
manual/advisory markers, include file:line and implementer handoff cues, expose
the queue limit and omitted count, and cross-check against
`manual-candidates.json`. The verifier must also reject implementer handoff
drift: target, route, invariant, external evidence commands, limitations, and
candidate-specific fix options, test targets, non-goals, and stop lines must
still project from the imported manual candidate. It is not the
ReviewCard repair queue.

When imported candidates are present in a `first-pr` run, `pr-summary.md` and
`github-summary.md` must include a compact manual-candidate front-door cue with
the manual count, advisory operation-family and evidence-kind count summaries,
first candidate ID, file:line, operation family, safe caller route, invariant,
evidence count, optional first-candidate fix/test/do-not-touch
guidance, a bounded manual-candidate queue preview with file:line, operation
family, evidence count, first guidance cue, copy-only context/witness-plan
commands, and advisory boundary. The full candidate payload remains in
`manual-candidates.json` and `review-kit.json`; the cue must not add manual
candidates to ReviewCard-only artifacts.

The bundled `first-pr` `witness-plan.md` may include a compact manual-candidate
follow-up cue that points to `candidate witness-plan` for the full copy-only
manual packet. That cue may include optional first-candidate fix/test/do-not-touch
guidance plus the bounded manual-candidate queue preview, must preserve the
manual/advisory markers, and must not add manual candidates to ReviewCard
witness route groups or import ReviewCard witness evidence.

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
- CLI list e2e coverage for `candidate list --format json`, Markdown output,
  sorted imported candidates, copy-only explain/context/witness-plan commands,
  optional fix options, test targets, do-not-touch guidance, and
  ReviewCard-only artifact relationship wording
- projection tests proving advisory operation-family and evidence-kind summary
  maps are derived from imported manual candidates and stay aligned across
  candidate list, first-pr `manual-candidates.json`, `review-kit.json`, and
  Markdown front-door cues
- projection tests proving `source = manual` and `manual_candidate = true` are
  preserved with `analyzer_discovered = false` in explain, context,
  witness-plan, saved JSON, first-pr `manual-candidates.json`, and outcome
  surfaces
- projection tests proving optional fix options, test targets, and do-not-touch
  guidance stay aligned across candidate import, explain/context, witness-plan,
  first-pr `manual-candidates.json`, and `review-kit.json`
- a checked smoke that imports committed manual-candidate examples into a
  disposable first-pr root and verifies the resulting advisory bundle
- receipt tests for manual candidate IDs
- negative tests proving manual candidates are not labeled analyzer-discovered

## Acceptance Examples

- Importing a valid manual candidate JSON writes a canonical candidate artifact
  with the same ID, source marker, location, operation family, unsafe operation,
  invariant, safe caller, evidence references, optional fix/test/non-goal
  guidance, and trust boundary.
- `explain` and `context` for a manual candidate state that it is manual and
  advisory, and they include the external evidence packet without claiming that
  unsafe-review found the issue.
- `context` and `witness-plan` carry an implementer handoff that names the
  file:line, safe caller route, invariant at risk, external evidence references,
  evidence commands and limitations, candidate-specific fix options, test
  targets, non-goals, and stop line from the same manual candidate.
- `candidate list` reports imported candidates as a manual/advisory ledger with
  sorted IDs, advisory operation-family and evidence-kind summaries, file:line
  locations, compact implementer handoff cues, evidence counts, optional fix
  options, test targets, do-not-touch guidance, copy-only projection commands,
  and ReviewCard-only artifact boundaries.
- `witness-plan` routes manual evidence as suggested follow-up work without
  executing witnesses.
- A receipt against a manual candidate ID can be imported or audited only as
  evidence for that manual candidate.
- Receipt audit output for a matched manual candidate preserves safe-caller
  route, invariant, evidence command/limitation, optional fix/test/non-goal
  guidance, and manual trust-boundary cues without importing ReviewCard witness
  evidence.
- Outcome comparison accepts both single manual candidate artifacts and
  aggregate manual candidate indexes, preserves manual source markers, and
  compares manual IDs deterministically across snapshots.
- `first-pr` writes `manual-candidates.json` for imported candidates and keeps
  ReviewCard-derived artifacts, including cards JSON, SARIF, comment-plan,
  saved LSP, repair queue, and policy-report surfaces, ReviewCard-only.
- The first-pr verifier rejects manual-candidate markers in ReviewCard-only
  artifacts instead of silently accepting leaked manual candidates as analyzer
  output.
- `manual-candidates.json` and the `review-kit.json` manual candidate handoff
  carry structured ReviewCard-only applicability metadata for SARIF,
  comment-plan, saved-LSP, repair-queue, and cards, plus policy-report
  follow-up metadata and advisory candidate-mix summary maps.
- `first-pr` terminal output and `review-kit.json` include a bounded,
  copy-only manual candidate handoff with `manual-candidates.json`,
  a sorted bounded candidate queue, `explain`, `context --json`, and
  `candidate witness-plan` commands while preserving `source = manual`,
  `manual_candidate = true`, and `analyzer_discovered = false`.
- `first-pr` `pr-summary.md` and `github-summary.md` show a compact manual
  candidate front-door cue, including advisory operation-family/evidence-kind
  summaries, a bounded queue preview, and optional guidance when present, so
  reviewers can notice and open the copy-only handoff without treating
  candidates as analyzer ReviewCards.
- `first-pr` `witness-plan.md` shows a compact manual candidate follow-up cue
  before the ReviewCard trust boundary, points to `candidate witness-plan`,
  includes a bounded queue preview plus optional guidance when present, and
  keeps manual candidates out of ReviewCard witness route groups.

## CI Proof

Current runtime proof:

```bash
cargo run --locked -p xtask -- check-docs
cargo run --locked -p xtask -- check-doc-artifacts
cargo run --locked -p xtask -- check-spec-status
cargo test -p unsafe-review-core manual_candidate
cargo test -p unsafe-review-core outcome
cargo test -p unsafe-review manual_candidate
cargo test -p unsafe-review first_pr_writes_standard_advisory_review_bundle
cargo test -p xtask manual_candidate
cargo run --locked -p xtask -- check-manual-candidate-examples
cargo run --locked -p xtask -- check-pr
cargo run --locked -p xtask -- source-divergence
git diff --check
```

## Metrics / Promotion Rule

Remain partial-runtime until applicability for policy reports is explicitly
accepted or rejected. SARIF, comment-plan, saved-LSP, and repair-queue first-pr
exports are currently rejected as ReviewCard-only surfaces for manual
candidates. The live runtime proof must keep import, explain, context,
witness-plan, receipt, and outcome projections preserving the manual candidate
source marker.

## Failure Modes

- imported candidates appear indistinguishable from analyzer findings
- receipts become the only durable artifact for manual discoveries
- projections drop external evidence references or manual trust-boundary text
- outcome comparison conflates manual and analyzer-discovered cards
