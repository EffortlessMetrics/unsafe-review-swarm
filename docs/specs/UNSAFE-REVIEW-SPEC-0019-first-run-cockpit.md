# UNSAFE-REVIEW-SPEC-0019 — 0.2.0 first-run cockpit

Status: proposed for 0.2.0 usability lane

## Goal

`unsafe-review` 0.2.0 MUST optimize for first-run unsafe PR review usability, not analyzer breadth.

The canonical first-run flow is:

```bash
cargo install unsafe-review --locked
unsafe-review doctor
unsafe-review first-pr --base origin/main
open target/unsafe-review/pr-summary.md
unsafe-review explain <card-id>
```

## Product statement

The 0.2.0 release MUST make `unsafe-review` feel like a first-run cockpit for changed unsafe seams:

- what changed
- which obligation matters
- what evidence exists
- what evidence is missing
- what action to ask from the author
- which witness route is worth running
- what `unsafe-review` is not claiming

## Required user-visible bundle

`first-pr` MUST write an advisory bundle at `target/unsafe-review/` with:

- `cards.json`
- `pr-summary.md`
- `cards.sarif`
- `comment-plan.json`
- `witness-plan.md`
- `lsp.json` (when generation succeeds; optional by verifier contract)

## Required terminal handoff

`first-pr` terminal output MUST be concise and include:

- artifact directory path
- changed-card count
- top-card id (if any)
- `pr-summary.md` path
- `unsafe-review explain <card-id>` handoff (if any)
- trust boundary text

The terminal summary MUST preserve this boundary:

- advisory static unsafe contract review only
- not a memory-safety proof

## Explain command requirements

`unsafe-review explain <card-id>` MUST be reviewer-first and render these sections in human output:

1. Why this card exists
2. Required safety conditions
3. Evidence found
4. Evidence missing
5. What would resolve this
6. What would not resolve this
7. Witness route
8. Trust boundary

`explain` JSON and human output MUST remain semantically aligned so downstream surfaces (saved LSP hover, agent packet templates, docs examples) do not diverge.

## No-card honesty contract

When no changed gaps exist, all applicable surfaces MUST use equivalent wording:

- `No changed unsafe-review gaps were found.`
- `This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.`

Forbidden phrasing includes unqualified success claims such as `All clear.`

## Doctor and support posture

`unsafe-review doctor` MUST act as a first-run readiness check and report:

- workspace/git/base-ref/cargo/artifact readiness
- witness tool availability (informational)
- policy mode advisory status
- trust boundary

Witness tool absence MUST NOT be treated as fatal for first-run review.

`unsafe-review support` MUST provide a compact non-overclaim posture panel for major surfaces (cards, bundle, receipts, outcomes, policy, blocking defaults, witness execution defaults, live LSP status).

## Witness-plan requirements

`witness-plan.md` MUST be reviewer-readable and grouped by route class:

- Miri / cargo-careful
- Sanitizers
- Loom / Shuttle
- Kani / Crux
- Human deep review
- Unsupported / manual

Each planned witness entry SHOULD include:

- card id
- why this route
- suggested command
- what it can show
- what it cannot prove
- receipt template hint

## Policy and receipts for 0.2.0

Receipt and policy surfaces in this lane MUST remain advisory and schema-pinned.

Receipt import/audit and policy report features MUST NOT imply:

- soundness proof
- UB-free status
- Miri-clean status
- calibrated precision/recall
- default blocking suitability

## Explicit non-goals for 0.2.0

The following MUST stay out of scope for the 0.2.0 release lane:

- default blocking CI
- automatic comment posting
- automatic source edits
- witness execution by default
- live LSP server
- VS Code/Open VSX extension as a release requirement
- broad suppressions
- precision/recall or policy-calibration claims

## Release proof requirements

0.2.0 readiness proof MUST include smoke evidence for:

- install path
- `doctor`
- `first-pr`
- `explain`
- `support`
- first-pr artifact verifier

## Lane completion definition

The lane is complete when first-run confidence is true:

- maintainer can install, run doctor, run first-pr, open summary, explain top card
- maintainer receives concrete next action for guard/test/witness
- outputs remain advisory and do not overclaim safety
