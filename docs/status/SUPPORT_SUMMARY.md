# Support summary

Date: 2026-08-08

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
| First-run CLI path: `doctor`, `pr` (with `first-pr` / `review` compatibility aliases), `explain`, `context`, and saved artifacts | Experimental | Fixture-backed CLI e2e coverage and current release-readiness proof; public v0.3.8 is the last published release | Proof, policy authority, source edits, witness execution, agent execution, or live editor integration |
| PR artifacts: review-kit manifest, bounded ReviewCard queue preview, cards JSON, PR summary, bounded GitHub summary, SARIF, comment-plan, witness-plan, receipt audit, manual-candidates JSON, manual repair queue sidecar, tokmd packet input sidecar, saved LSP JSON, and repair queue | Experimental | Fixture-backed and workflow-verified; advisory artifact loop is dogfoodable | Automatic comments, branch protection, witness execution, source edits, agent execution, repair success, rendered tokmd output, or policy gating |
| Saved LSP projection and agent packet | Experimental | Fixture-backed and e2e-covered read-only projections from `ReviewCard` | Live editor integration, agent execution, source edits, or repair success |
| Witness routes and saved-output receipt import | Experimental | Fixture-backed route table plus saved-output receipt adapters | Running Miri/cargo-careful/sanitizers/Loom/Kani, site reach, or witness success beyond imported receipt scope |
| Manual candidate ledger | Experimental | Fixture-backed import, explain/context, witness-plan, receipt audit, saved-outcome projection, oracle-map, proof-mode, fix-boundary, and PR-aperture preservation for advisory manual candidates | Analyzer discovery, proof, witness execution, site execution, repository safety, or policy authority |
| Repo posture, badge JSON, outcome comparison, and policy report | Experimental | Fixture-backed; outcome comparison has limited saved-snapshot dogfood | Safety badge, release-grade dashboard, default no-new-debt, or blocking policy |
| Real-crate dogfood measurement | Experimental | Seven capped repo snapshots and twenty-three selected PR diffs across real crates | Calibrated rates, full audits, uncapped performance guarantees, or ecosystem-wide coverage |
| MIR/nightly facts and live LSP/editor integration | Deferred or planned | Requires later ADR/spec and proof | Default dependency, support promise, or v0.x gate behavior |

## Public and candidate posture

The last public release is `v0.3.8` (2026-06-18). The next candidate is still
unfrozen: its version, candidate SHA, dependency freeze, and publication state
remain unset in the [draft release cutline](RELEASE_CUTLINE.md). Do not read
Swarm main integration as public availability or qualification.

The current first-use contract is install → `doctor` → `pr` → reviewer front
panel → `explain`/`context` or human review → named external verification. The
preview-only baseline slice does not make a completed top-level `init` command
part of the supported path.

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
