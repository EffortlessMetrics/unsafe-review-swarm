# UNSAFE-REVIEW-SPEC-0012: LSP and editor projection

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md

## Problem

`unsafe-review` needs a precise, checkable behavior contract for lsp and editor projection.

## Behavior

LSP v1 is read-only: diagnostics, hover, status, copy agent packet, copy witness command without running it, open related test.
The first supported surface is a saved JSON projection rendered with
`unsafe-review check --format lsp`. It is not an LSP server and it does not
edit source. The projection derives diagnostics, hovers, and command-style
action data from existing `ReviewCard`s. Diagnostics carry structured next
action, witness route details, verify commands, missing evidence, required
safety conditions, obligation-level evidence states, and the static-review
trust boundary. Diagnostics also include the concrete operation expression from
the `ReviewCard`, so editor consumers do not need to parse hover text or
reclassify findings. Hover text is a compact reviewer view: card identity,
human-readable location, relevant hazard families, required safety conditions,
ReviewCard evidence summaries, missing evidence, next action, what would not
resolve the card,
including that widening unsafe scope, suppressing the card, or changing
unrelated unsafe code is not a resolution; verify commands when available;
witness route; card-scoped `explain` and `context --json` handoff commands; and
the static-review trust boundary. Code actions include stable object `payload`
fields with `card_id` plus action-specific details so editor adapters do not
need to parse positional legacy arguments.

## Projection contract

For 0.2.0, the editor surface is the saved `lsp.json` projection. A live LSP
server remains a later lane until the live-server hardening gate in SPEC-0018 is
met.

All editor diagnostics, hovers, and actions project from `ReviewCard`. They must
not reclassify cards, invent separate evidence, or parse prose to recover
machine fields.

Code actions are command-only. They may copy a bounded agent packet, copy a
witness command without running it, explain a witness route, or open a
statically related test.
They must not contain `WorkspaceEdit`, apply patches, insert SAFETY comments,
execute witnesses, post comments, or enforce policy. This applies to both the
top-level action object and nested command payloads.
Witness-command actions must copy only commands already projected from the same
ReviewCard's verify commands.

The canonical action producer is
`output/lsp/action_contract.rs::actions_for_card`. It owns stable action IDs,
hierarchical kinds, diagnostic/card association, analysis identity,
agent-readiness wording, applicability, disabled reason codes, and the explicit
command-only posture. Saved and live adapters preserve those semantics rather
than deriving them from titles. The closed card-action IDs are `agent-packet`,
`witness-route`, `witness-command`, and `related-test`.

Unavailable route, witness-command, or related-test capabilities are omitted or
represented with the canonical disabled reason; an enabled action must not
execute to an unexplained null. Human-review actions remain non-preferred and
must not imply automatic repair authority. Saved `lsp.json` schema 0.2 carries
the canonical action objects and full analysis identity. Consumers may render
legacy 0.1 actions for compatibility, but must not mix the two action shapes or
recover executable 0.2 semantics from titles and positional arguments.

Every diagnostic and card-scoped action must carry the relevant `card_id` and
the static-review trust boundary. Diagnostic data must include the operation
identity, required safety conditions, obligation-level evidence states, missing
evidence, witness routes, and verify commands.

Diagnostic `severity` is a ReviewClass projection. It must use the same class
table as SARIF `level`, must not be derived from priority, operation family,
baseline state, or comment eligibility, and must not emit LSP `Error` severity
for the default advisory projection.

## Non-goals

- no soundness claim
- no hidden blocking unless policy mode explicitly enables it
- no duplicate truth outside this spec and linked policy files

## Required evidence

- fixture-backed examples for positive and negative cases
- JSON output contract coverage
- human output smoke coverage
- policy documentation when behavior is configurable

## Acceptance examples

- A changed unsafe seam produces one review card with stable identity.
- The card includes missing evidence and a next action.
- If evidence is not knowable statically, the card names the limitation instead of overclaiming.
- `unsafe-review check --format lsp --out target/unsafe-review/lsp.json`
  writes read-only status data, diagnostics, hovers, and copy-command action
  data, including opening statically related tests when test-reach evidence is
  present.
- The projection includes no source edits and preserves the static-review trust
  boundary.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
cargo test -p unsafe-review-core lsp_projection
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
