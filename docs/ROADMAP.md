# Roadmap

## 0.1.0 — Static-first review cards

- stable-only source scanner
- review card schema
- CLI: `check`, `repo`, `pilot`, `badges`, `doctor`, `explain`, `context`
- hazard and obligation taxonomy v1
- contract/discharge evidence mining v1
- human / JSON / Markdown output
- specification and policy system

## 0.2.0 — First-run cockpit usability

- first-run cockpit flow (`doctor` -> `first-pr` -> `pr-summary.md` -> `explain`)
- advisory PR bundle (`cards.json`, `pr-summary.md`, `cards.sarif`, `comment-plan.json`, `witness-plan.md`, optional `lsp.json`)
- concise first-pr terminal handoff to summary and top-card explain
- reviewer-first explain layout with explicit resolve/do-not-resolve guidance
- first-run doctor readiness checks and support posture surface
- first-pr artifact verification and release smoke proof
- strict no-overclaim trust boundary across all outputs

## 0.3.0 — LSP and agent workflow

- saved-workspace LSP diagnostics
- hover cards
- code actions for copying packets and witness commands
- copy-only bounded agent packets

## 0.4.0 — Repo posture and policy

- baseline and suppression matching
- no-new-debt mode
- repo inventory hardening
- badge output hardening
- outcome comparison

## 0.5.0 — Witness receipts

- receipt import for Miri, cargo-careful, sanitizers, Loom, Kani, and Crux
- witness-plan artifacts

## 0.6.0 — Calibration and promotion

- fixture-backed calibration corpus
- false-positive tracking
- dogfood-calibrated evidence loop
- real-crate dogfood corpus manifest and validation
- saved-snapshot outcome reasons and receipt movement
- non-blocking advisory policy reports
- support-tier promotion rules
- optional nightly/MIR fact adapter ADR
- repo outcome comparison

## Deferred

- automatic code fixes
- generated tests
- rustc_private/MIR dependency in the product binary
- blocking gate defaults
