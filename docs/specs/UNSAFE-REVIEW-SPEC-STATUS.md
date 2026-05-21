# Spec lifecycle status dashboard

This dashboard is an operator view over specification lifecycle state.

Until an automated generator lands, keep this file aligned with the current
active goal, implementation plan, and linked closeouts.

| Spec | Status | Implementation state | Proof commands | Last touched | Notes |
|---|---|---|---|---|---|
| `UNSAFE-REVIEW-SPEC-0019` first-run cockpit | accepted, release-prepped | Source and swarm lanes promoted; publication receipt documented | `cargo run --locked -p xtask -- check-first-pr-artifacts`; first-run smoke from release prep | 2026-05-21 | Reviewer-first cockpit contract for `doctor`, `first-pr`, `explain`, bundle honesty |
| `UNSAFE-REVIEW-SPEC-0020` source-of-truth stack | accepted, active-maintenance | Artifact taxonomy and linkage landed; current lane still tracks source-of-truth operations | `cargo run --locked -p xtask -- check-doc-artifacts`; `cargo run --locked -p xtask -- check-goals`; `cargo run --locked -p xtask -- check-ci-lanes`; `cargo run --locked -p xtask -- source-divergence` | 2026-05-21 | Requires active goal/plan freshness so the repo answers “what next?” without chat context |
| `UNSAFE-REVIEW-SPEC-0012` LSP/editor projection | accepted, partial-runtime | Saved projection contract promoted; live runtime remains limited to swarm lanes | `cargo run --locked -p xtask -- check-first-pr-artifacts`; projection contract/doc checks in `check-pr` | 2026-05-21 | Keep “no overclaim” boundary: saved projection is product truth; live server rollout is separate |

## Reading notes

- **Status** is specification lifecycle intent (draft/accepted/etc.).
- **Implementation state** describes repository reality, including promotions and deferrals.
- **Proof commands** are the minimum commands that must stay green for the listed claim posture.

## Follow-up

Add/land an `xtask` dashboard check so this table becomes machine-validated rather
than purely editorial.
