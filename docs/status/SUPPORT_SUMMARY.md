# Support summary

Date: 2026-05-18

This is the front panel for current `unsafe-review` support posture. The full
claim-to-proof ledger remains [`SUPPORT_TIERS.md`](SUPPORT_TIERS.md).

All surfaces describe static unsafe-review evidence. None of them is a
memory-safety proof, UB-free claim, Miri-clean claim, target-feature availability
proof, site-execution proof, or calibrated policy gate.

## Proof Vocabulary

| Label | Meaning |
|---|---|
| Fixture-backed | Covered by curated fixtures, goldens, unit tests, or e2e tests. |
| Dogfood-backed | Exercised on selected real crates or PR diffs with recorded limits. |
| Calibrated | Measured across a documented corpus with known false-positive and false-negative behavior. |

No current surface is calibrated. Blocking policy remains out of scope until
calibration exists and support tiers are explicitly promoted.

## Current Posture

| Surface | Current posture | Evidence | Not claimed |
|---|---|---|---|
| ReviewCard schema, identity, and core card slices | Experimental | Fixture-backed; selected analyzer rules are dogfood-backed | Stable schema compatibility, broad precision/recall, or safety |
| PR artifacts: cards JSON, PR summary, SARIF, comment-plan | Experimental | Fixture-backed and workflow-verified; advisory artifact loop is dogfoodable | Automatic comments, branch protection, witness execution, or policy gating |
| Saved LSP projection and agent packet | Experimental | Fixture-backed and e2e-covered read-only projections from `ReviewCard` | Live editor integration, agent execution, source edits, or repair success |
| Witness routes and saved-output receipt import | Experimental | Fixture-backed route table plus saved-output receipt adapters | Running Miri/cargo-careful/sanitizers/Loom/Kani, site reach, or witness success beyond imported receipt scope |
| Repo posture, badge JSON, outcome comparison, and policy report | Experimental | Fixture-backed; outcome comparison has limited saved-snapshot dogfood | Safety badge, release-grade dashboard, default no-new-debt, or blocking policy |
| Real-crate dogfood measurement | Experimental | Eight capped repo snapshots and twenty-three selected PR diffs across real crates | Calibrated rates, full audits, uncapped performance guarantees, or ecosystem-wide coverage |
| MIR/nightly facts and live LSP/editor integration | Deferred or planned | Requires later ADR/spec and proof | Default dependency, support promise, or v0.x gate behavior |

## Promotion Posture

- Fixture-backed surfaces may stay experimental until they survive dogfood.
- Dogfood-backed surfaces may describe the exact crates, PRs, and limits tested.
- Calibrated support requires measured outcomes, not just more fixtures or a
  larger support-tier table.
- Policy gating is not ready. `--policy no-new-debt` is explicit opt-in, and
  `policy report` is advisory-only.

## Core Boundary

`unsafe-review` finds unsafe Rust changes missing a safety contract, guard, test,
or witness. It routes reviewers to the cheapest credible next action; it does
not prove the repository safe.
