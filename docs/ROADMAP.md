# Roadmap

## 0.1.0 — Static-first review cards

- stable-only source scanner
- review card schema
- CLI: `check`, `repo`, `pilot`, `badges`, `doctor`, `explain`, `context`
- hazard and obligation taxonomy v1
- contract/discharge evidence mining v1
- human / JSON / Markdown output
- specification and policy system

## 0.2.0 — Public usability

- first-run `doctor` readiness check
- one-command `first-pr` / `review` advisory bundle
- readable PR summary, witness plan, and honest no-card states
- reviewer-first `explain <card-id>`
- support posture command
- first-pr artifact bundle verifier
- release target: [0.2.0 public usability](releases/0.2.0-public-usability.md)

## 0.3.0 — Editor-adjacent and agent workflow

- saved-workspace LSP diagnostics
- hover cards
- code actions for copying packets and witness commands
- copy-only bounded agent packets
- live LSP remains deferred until saved artifacts and first-run UX are solid

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
