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
oracle_map
proof_mode
fix_boundary
pr_aperture
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
- `proof_mode`: optional advisory proof-mode object for candidate packets that
  need an explicit proof bar before implementation or ledger movement.
- `oracle_map`: optional cross-language oracle object for candidates whose
  actionable test, witness, or comparison lives outside the Rust seam. It must
  include `rust_seam`, `oracle_language`, `oracle_path`, `oracle_kind`,
  `coverage_confidence`, and `limitation`; the limitation must state the oracle
  map is not witness execution, site-execution proof, or memory-safety proof.
- `fix_boundary`: optional copy-only statement of the smallest repair boundary
  to try first.
- `pr_aperture`: optional copy-only statement of the intended upstream PR scope
  and stop line.
- `trust_boundary`: explicit manual/advisory boundary text.
- `fix_options[]`: optional copy-only implementer guidance for candidate-local
  repair approaches.
- `test_targets[]`: optional copy-only test or witness targets that should
  validate the candidate-local change.
- `do_not_touch[]`: optional copy-only non-goals that must stay out of the
  candidate-local change.
- `trust_boundary` must name the manual boundary and say the packet is not
  analyzer-discovered, not witness execution, not proof, not UB-free status,
  not Miri-clean status, not site-execution proof, and not policy readiness.

## Stable-Byte-Source Vocabulary

Large mixed-language repositories such as Bun may use manual candidates to
classify a stable-byte-source risk before the analyzer can discover the route.
This is advisory classification vocabulary for manual packets and cockpit
handoffs. It is not analyzer discovery, witness execution, proof of UB, proof of
site execution, or a policy signal.

The stable-byte invariant is:

```text
Rust/native code must not retain or later materialize a &[u8] or pointer+len
from JS-owned storage after JS can mutate, resize, detach, replace, race, or
reenter that backing storage.
```

Manual stable-byte candidates should use one of these classes when the route
matches:

| Class | Source shape | Sink shape | Hazard |
|---|---|---|---|
| `stable-byte-source-rab-async` | JS `TypedArray`, `DataView`, or buffer view over resizable or detachable storage | async worker, deferred native write/read, or scheduled Rust/native use | cached pointer/len can outlive backing storage validity after resize, detach, replacement, or mutation |
| `stable-byte-source-sab-race` | SharedArrayBuffer, growable shared backing storage, or shared view | Rust slice, decoder, parser, native read, or FFI call that treats bytes as stable | shared bytes can mutate concurrently while Rust/native code observes them as immutable or stable |
| `stable-byte-source-getter-reentry` | JS descriptor/options/path capture followed by getter, callback, or other JS reentry | later Rust/native byte read, compression/decompression, decode, path use, or FFI read | JS reentry can mutate, resize, detach, or replace the backing storage between capture and use |
| `stable-byte-source-helper-dependent` | route stability depends on a helper such as clone, pin, snapshot, byte-copy, or coercion | later sink is safe only if the helper actually stabilizes bytes before use | proof and fix boundary are blocked on exact helper semantics |
| `stable-byte-source-pathlike-live-view` | pathlike or stringlike bytes supplied through a JS-owned live view | filesystem, module loading, URL/path conversion, or native path API | path bytes may be read after JS can mutate or replace the view that supplied them |
| `stable-byte-source-native-ffi-read` | JS-backed or otherwise movable bytes passed as pointer+len toward native code | native library, syscall wrapper, C/C++ function, or Zig/C FFI read | native code can read through a pointer/len whose lifetime or backing storage stability is not established |

Stable-byte manual packets should project these fields when known. Runtime JSON
must avoid colliding with the top-level `source = manual` marker; implementations
should namespace the byte-source fields, for example under `stable_byte`, while
human cockpit views may render the labels below.

| Field | Meaning |
|---|---|
| `class` | one stable-byte-source class from the table above |
| `source` | concrete caller-visible byte source, for example JS `TypedArray` over a resizable ArrayBuffer |
| `sink` | Rust/native seam that reads or retains the bytes |
| `hazard` | candidate-specific stable-byte invariant at risk |
| `observable` | whether the candidate has expected system-visible wrong behavior (`yes`), is nondiscriminating/non-observable (`no`), is route-only (`source-route-only`), or is blocked on helper semantics (`helper-gated`) |
| `proof_required` | one of `observable-red-green`, `mutation-plus-miri`, `source-route-only`, or `helper-gated` |
| `suggested_fix_boundary` | smallest byte-stability boundary to try first, such as snapshot before scheduling, parse options before capture, or re-fetch/copy after getter reentry |
| `pr_aperture` | intended upstream PR scope plus explicit stop line, such as scalar write only and not writev/pathlike |
| `ledger_state` | workflow state from the stable-byte ledger vocabulary below |

Proof mode selection must stay explicit:

- `observable-red-green`: use when wrong behavior is expected through a safe
  system route; require system-Bun red and patched-green evidence before
  claiming the candidate is fixed.
- `mutation-plus-miri`: use when system behavior is nondiscriminating or UB is
  not directly observable; require a mutation witness plus a Miri/model proof of
  the aliasing or lifetime shape before upgrading confidence.
- `source-route-only`: use when the packet currently proves only the safe caller
  route and sink seam; do not label the candidate as sure UB from source
  inspection alone.
- `helper-gated`: use when a helper's exact copy/pin/snapshot semantics decide
  the candidate; park as a verified follow-up with the exact unblock command or
  source check.

Stable-byte ledger states are workflow primitives, not proof claims:

| State | Meaning |
|---|---|
| `handoff-ready` | source-routed manual packet exists and the next implementer action is clear |
| `fork-draft` | fix is implemented in a fork or worktree and still under local/fork validation |
| `upstream-open` | smallest upstreamable PR is open and maintainer-review gated |
| `parked-followup` | work is done and verified but not upstreamable until a named dependency or helper decision lands |
| `merged-upstream` | upstream PR landed and ledger can retain receipt/provenance |
| `needs-refresh` | upstream/main or the fork delta moved and the route/proof/patch needs recheck |

Stable-byte packets must continue to use the normal manual-candidate trust
boundary:

```text
source = manual
manual_candidate = true
analyzer_discovered = false
```

They must also state what unsafe-review is not claiming. In particular,
source-route-only evidence is not proof of UB, an observable witness is not
Miri-clean status, a model is not site execution, and ledger state is not
policy readiness.

Each `evidence[]` item must include:

- `kind`: closed vocabulary such as `runtime_witness`, `model`,
  `source_trace`, `node_parity`, `human_review`, or `other`.
- `path`: local artifact path, when evidence is file-backed.
- `summary`: optional concise description of what the evidence supports.
- `command`: optional exact external command that produced the evidence.
- `limitation`: optional concise statement of what the evidence does not prove.

When present, `proof_mode` must include:

- `kind`: one of `observable-red-green`, `mutation-plus-miri`,
  `source-route-only`, or `helper-gated`.
- `system_bun_expected`: one of `fail`, `nondiscriminating`, or `unavailable`.
- `mutation_required`: boolean.
- `miri_required`: boolean.

The importer must preserve `proof_mode`, `fix_boundary`, and `pr_aperture` in
canonical manual candidate JSON, candidate list JSON, manual candidate
`context --json`, outcome comparison, receipt audit, and first-pr manual
candidate ledger surfaces. These fields remain copy-only advisory handoff
fields. They must not convert a candidate into proof, policy readiness, witness
execution, or analyzer discovery.

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
  "proof_mode": {
    "kind": "mutation-plus-miri",
    "system_bun_expected": "nondiscriminating",
    "mutation_required": true,
    "miri_required": true
  },
  "fix_boundary": "Snapshot shared/growable/resizable bytes before Rust receives &[u8]",
  "pr_aperture": "TextDecoder shared-byte snapshot only; do not patch S3, fs, writev, or unrelated encodings",
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
  "trust_boundary": "manual candidate; not analyzer-discovered; not witness execution; not proof of memory safety; not UB-free status; not Miri-clean status; not site-execution proof; not policy readiness"
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
surfaces with `decision = reviewcard_only`, and must include explicit
`policy-report.json` and `policy-report.md` entries with
`decision = reviewcard_only`. All entries must keep
`applies_to_manual_candidates = false` and
`manual_candidate_markers_allowed = false`. They must not add manual candidates
to `cards.json`, SARIF, comment-plan, saved LSP, repair-queue, or policy-report
surfaces.
The first-pr artifact verifier must reject manual-candidate marker leakage
(`source = manual`, `manual_candidate`, or `analyzer_discovered`) in the
ReviewCard-only first-pr artifacts: `cards.json`, `cards.sarif`,
`comment-plan.json`, `lsp.json`, `repair-queue.json`, `policy-report.json`,
and `policy-report.md`.

`review-kit.json` may include a bounded `candidate_queue` under the manual
candidate handoff so reviewers and agents can see more than the first imported
candidate. That queue must stay copy-only, preserve sorted manual IDs and
manual/advisory markers, include file:line and implementer handoff cues, expose
the queue limit and omitted count, and cross-check against
`manual-candidates.json`. The handoff may also include candidate-mix summaries
for proof modes, stable-byte-source classes, ledger states, and optional
oracle-map/fix-boundary/PR-aperture/guidance presence counts so reviewers can
see whether the imported Bun queue is observable, model-heavy, helper-gated, or
handoff-ready without opening each packet. These summaries must derive only
from imported manual-candidate fields and must not become a second source of
truth. The verifier must also reject implementer handoff drift: target, route,
invariant, external evidence commands, limitations, and candidate-specific fix
options, test targets, non-goals, and stop lines must still project from the
imported manual candidate. It is not the ReviewCard repair queue.
When a root-local `docs/dogfood/stable-byte-follow-up-seeds.md` ledger exists
and joins by manual candidate ID, the review-kit manual candidate handoff may
also project `with_stable_byte_seed`, `stable_byte_seed_source`, and
per-candidate `stable_byte_seed` entries with seed ID, owner lane, suggested
first PR, safe JS caller route, Rust/native sink, triage labels, and
`candidate_consistency` flags for class, proof mode, ledger state, safe JS
caller route, and Rust/native sink. Seed rows are advisory workflow metadata only:
they are not analyzer discovery, not witness execution, not proof, not policy
readiness, and not a ReviewCard truth.

`first-pr` may also write `manual-repair-queue.json` as a dedicated
manual-candidate repair handoff sidecar. It must use
`schema_version = manual-repair-queue/v1`, `source = manual_candidate`,
`mode = manual_candidate_repair_queue`, and `policy = advisory`. It must
preserve sorted manual IDs, `source = manual`, `manual_candidate = true`, and
`analyzer_discovered = false`, project the same implementer handoff, guidance,
and copy-only explain/context/witness-plan commands as `manual-candidates.json`,
and cross-check summary counts and guidance against `manual-candidates.json`.
Its summary must expose proof-mode, stable-byte-source class, and ledger-state
count maps derived only from the imported manual candidate fields so a reviewer
can see observable, Miri/model, helper-gated, handoff-ready, or parked-followup
work at queue level without treating the sidecar as a second source of truth.
It is not `repair-queue.json`, not analyzer-discovered ReviewCard output, not
automatic repair, not proof, not witness execution, and not policy gating.
`review-kit.json` may include a `handoff.repair_queues` front panel that places
the checked ReviewCard `repair-queue.json` summary and the checked
manual-candidate `manual-repair-queue.json` summary side by side. This is only
a cockpit routing cue: it must keep `source = review_card` and
`source = manual_candidate` separated, cross-check counts against the two queue
artifacts, and must not merge manual candidates into ReviewCard repair queues
or imply automatic repair.

`first-pr` may also write `tokmd-packets.json` as a formatting-input sidecar
for future Bun packet presets. It must use
`schema_version = tokmd-packets/v1`, `source = first_pr`, and
`policy = advisory`. It must preserve sorted manual IDs, `source = manual`,
`manual_candidate = true`, and `analyzer_discovered = false` per packet,
project the same target, route, invariant, external evidence, optional
`oracle_map`, `proof_mode`, `fix_boundary`, `pr_aperture`, implementer
handoff, and copy-only commands as `manual-candidates.json`, include a
`manual_repair_queue_item` projection from `manual-repair-queue.json` with the
candidate ID, bucket, bucket reason, copy-only agent handoff, and trust
boundary, and may include `preset_inputs` keyed by the future Bun packet
presets (`bun-ub-handoff`, `bun-ub-pr-body`, `bun-ub-ledger-note`,
`bun-ub-review-map`, and `bun-ub-next-pick`). `preset_inputs` must be derived
from the same manual candidate, joined stable-byte seed row, manual repair
queue item, and bundle-level comment-plan relationship; it is a copy-only
formatting input, not rendered tokmd output and not a second source of truth.
The preset input must preserve implementer route/proof/fix/non-goal fields,
PR-body non-claim fields, ledger transition limits, review-map no-posting
boundaries, and next-pick proof action without running tokmd or selecting
comments. `tokmd-packets.json` may project the ReviewCard-only
`comment-plan.json` review-budget summary plus selected/not-selected
reason-code counts for future `bun-ub-review-map` formatting, and must record
absent ReviewCard, receipt, and stable-byte seed-ledger inputs as explicit
limitations. The comment-plan projection must remain plan-only and must not
select manual candidates for comments or imply posting. When a root-local
`docs/dogfood/stable-byte-follow-up-seeds.md` exists and its referenced manual
candidate JSON can be read, `tokmd-packets.json` may join a matching seed row
by manual candidate ID and project `stable_byte_seed` with seed ID, surface,
owner lane, suggested first PR, safe JS caller route, Rust/native sink, triage
labels, and `candidate_consistency` flags as advisory workflow metadata.
Packet-local `stable_byte.ledger_state` must be preserved when supplied and
must not be reported as a missing stable-byte ledger input. Seed rows are not a
second ReviewCard truth. They are not rendered tokmd output, not
analyzer-discovered ReviewCard output, not automatic repair, not proof, not
witness execution, and not policy gating.

When imported candidates are present in a `first-pr` run, `pr-summary.md` and
`github-summary.md` must include a compact manual-candidate front-door cue with
the manual count, advisory operation-family and evidence-kind count summaries,
first candidate ID, file:line, operation family, safe caller route, invariant,
evidence count, optional first-candidate fix/test/do-not-touch
guidance, optional proof mode, fix boundary, PR aperture, stop line, a bounded
manual-candidate queue preview with file:line, operation family, evidence
count, first guidance cue, copy-only context/witness-plan commands, a
`manual-repair-queue.json` cue naming it as a copy-only manual candidate repair
handoff separate from ReviewCard `repair-queue.json`, and advisory boundary.
The full candidate payload remains in
`manual-candidates.json` and `review-kit.json`; the cue must not add manual
candidates to ReviewCard-only artifacts.
When the first candidate carries `stable_byte`, the full reviewer cockpit and
witness follow-up cue must also render its class, observable/proof/ledger state,
source-to-sink route, and hazard so the cockpit remains useful without opening
raw JSON. The compact GitHub summary may collapse this to class, proof mode,
ledger state, source-to-sink route, and a sidecar pointer to keep the hosted
summary within its word budget.
When a joined stable-byte seed row is present, the front-door cue and bounded
queue preview render the seed ID, owner lane, suggested first PR, and triage
labels as next-lane workflow metadata. This must not replace the
candidate-local `stable_byte` packet fields or upgrade the candidate into a
ReviewCard finding.

The bundled `first-pr` `witness-plan.md` may include a compact manual-candidate
follow-up cue that points to `candidate witness-plan` for the full copy-only
manual packet. That cue may include optional first-candidate fix/test/do-not-touch
guidance, proof mode, fix boundary, PR aperture, stop line, plus the bounded
manual-candidate queue preview, must preserve the manual/advisory markers, and
must not add manual candidates to ReviewCard witness route groups or import
ReviewCard witness evidence.
If a joined stable-byte seed row is present, the follow-up cue renders the
same seed ID, owner lane, suggested first PR, and triage labels as advisory
workflow metadata only.

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
- projection tests proving review-kit manual-candidate proof-mode,
  stable-byte-source-class, ledger-state, oracle-map, fix-boundary,
  PR-aperture, and guidance summary cues stay derived from imported manual
  candidates and are verifier-checked
- projection tests proving `source = manual` and `manual_candidate = true` are
  preserved with `analyzer_discovered = false` in explain, context,
  witness-plan, saved JSON, first-pr `manual-candidates.json`, and outcome
  surfaces
- projection tests proving optional fix options, test targets, and do-not-touch
  guidance stay aligned across candidate import, explain/context, witness-plan,
  first-pr `manual-candidates.json`, and `review-kit.json`
- schema and projection tests proving optional `proof_mode`, `fix_boundary`,
  and `pr_aperture` fields are validated, preserved in canonical candidate
  JSON, and visible in candidate context, outcome, receipt audit, and first-pr
  manual-candidate sidecar surfaces without becoming ReviewCard evidence
- verifier and first-pr e2e tests proving `manual-repair-queue.json` stays
  aligned with `manual-candidates.json`, preserves manual markers and
  implementer guidance, and remains separate from ReviewCard `repair-queue.json`
- outcome projection tests proving single-candidate and aggregate
  `manual-candidates/v1` comparisons preserve safe caller, invariant, external
  evidence command/limitation, optional fix/test/non-goal guidance, and manual
  advisory markers without importing ReviewCard witness evidence
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
- `context`, outcome comparison, receipt audit, and manual candidate first-pr
  sidecars preserve optional proof mode, fix boundary, and PR aperture fields as
  advisory handoff metadata.
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
  compares manual IDs deterministically across snapshots while keeping the
  safe-caller route, invariant, evidence command/limitation, and optional
  fix/test/non-goal guidance visible as manual advisory handoff context.
- `first-pr` writes `manual-candidates.json` for imported candidates and keeps
  ReviewCard-derived artifacts, including cards JSON, SARIF, comment-plan,
  saved LSP, repair queue, and policy-report JSON/Markdown surfaces,
  ReviewCard-only.
- `first-pr` writes `manual-repair-queue.json` as a separate copy-only manual
  candidate repair handoff, preserving manual markers, guidance, and commands
  from `manual-candidates.json` while keeping ReviewCard `repair-queue.json`
  free of manual-candidate markers.
- `first-pr` writes `tokmd-packets.json` as a formatting-input sidecar,
  preserving manual markers, optional oracle-map,
  proof-mode/fix-boundary/PR-aperture guidance, implementer handoff, commands
  from `manual-candidates.json`, and
  `manual_repair_queue_item` data from `manual-repair-queue.json`, plus
  per-preset Bun packet `preset_inputs`, optional ReviewCard-only comment-plan
  review-budget metadata, and optional root-local stable-byte seed-row metadata
  while recording absent seed-ledger, receipt, and ReviewCard packet inputs as
  limitations and not running tokmd or posting comments.
- The first-pr verifier rejects manual-candidate markers in ReviewCard-only
  artifacts instead of silently accepting leaked manual candidates as analyzer
  output.
- `manual-candidates.json` and the `review-kit.json` manual candidate handoff
  carry structured ReviewCard-only applicability metadata for SARIF,
  comment-plan, saved-LSP, repair-queue, cards, and policy-report JSON/Markdown
  artifacts, plus advisory candidate-mix summary maps including proof-mode,
  stable-byte-source-class, ledger-state, oracle-map, guidance counts, and
  optional stable-byte seed-row source/count metadata.
- `first-pr` terminal output and `review-kit.json` include a bounded,
  copy-only manual candidate handoff with `manual-candidates.json`,
  a sorted bounded candidate queue, `explain`, `context --json`, and
  `candidate witness-plan` commands, plus optional joined stable-byte seed ID,
  owner lane, suggested first PR, and triage labels when a root-local seed row
  exists, while preserving `source = manual`, `manual_candidate = true`, and
  `analyzer_discovered = false`.
- `review-kit.json` includes a checked `handoff.repair_queues` front panel that
  shows ReviewCard repair-queue counts and manual-candidate repair-queue counts
  side by side while keeping both source ledgers separate.
- `first-pr` `pr-summary.md` and `github-summary.md` show a compact manual
  candidate front-door cue, including advisory operation-family/evidence-kind
  summaries, a bounded queue preview, optional proof mode, fix boundary, PR
  aperture, stop line, optional guidance, and optional joined stable-byte seed
  owner/next-PR metadata when present, so reviewers can notice and open the
  copy-only handoff without treating candidates as analyzer ReviewCards.
- `first-pr` `witness-plan.md` shows a compact manual candidate follow-up cue
  before the ReviewCard trust boundary, points to `candidate witness-plan`,
  includes a bounded queue preview plus optional proof mode, fix boundary, PR
  aperture, stop line, guidance, and optional joined stable-byte seed
  owner/next-PR metadata when present, and keeps manual candidates out of
  ReviewCard witness route groups.

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
cargo test -p xtask first_pr_artifact_checker
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
