# Dogfood triage taxonomy

Status: experimental dogfood vocabulary

This taxonomy keeps dogfood reports focused on reviewer usefulness instead of
raw card counts. It is a reporting aid for `unsafe-review-swarm` workbench
runs, not a support-tier promotion, calibration report, policy decision,
precision claim, recall claim, safety proof, UB-free claim, Miri-clean claim,
or site-execution proof.

Use exactly one primary label for each notable observation. Add a short note
when an observation has secondary implications.

## Labels

| Label | Use when | Typical next step | Do not infer |
|---|---|---|---|
| `actionable` | The card identifies a concrete changed unsafe-adjacent seam, missing or weak evidence, and a plausible reviewer action. | Preserve or improve projection quality; add a regression only if the behavior is fragile. | The code is unsafe, the witness route proves anything, or the card should block CI. |
| `noise` | The card is technically explainable but likely too broad, poorly ranked, duplicated, inventory-like, wrong-target, stale, or not helpful for the PR reviewer. | Add a false-positive control, ranking rule, operation classification, or wording fix. | The entire operation family is unsupported or useless. |
| `missed` | Manual inspection finds a changed unsafe-adjacent obligation that did not produce a useful ReviewCard. | Add one fixture-backed detector or route improvement. | Global recall, policy readiness, or safety status. |
| `needs-fixture` | The dogfood shape should become a small positive or negative fixture before changing analyzer behavior. | Extract the smallest reproducible fixture and one stale/wrong-target control. | The dogfood observation alone is enough to broaden detection. |
| `needs-doc` | The behavior is acceptable, but reviewer-facing wording, trust-boundary text, examples, or support-tier docs are unclear. | Update the owning docs/spec surface. | Analyzer behavior needs to change. |
| `needs-route` | The card is useful only if its witness or human-review route is more precise. | Adjust route wording or route classification with a fixture if possible. | A witness was executed or a route proves safety. |
| `needs-analyzer` | The observation points to a narrowly scoped analyzer gap that is ready for implementation. | Open one PR for one operation family, evidence shape, and false-positive control. | Broad analyzer expansion is justified. |
| `needs-verifier` | The behavior is acceptable, but a projection, artifact, receipt, or policy rail needs a checker so the contract cannot drift. | Add or tighten one verifier invariant with a focused regression. | Analyzer behavior needs to change, or the verifier proves safety. |

## Required fields in reports

For each triaged observation, include:

| Field | Requirement |
|---|---|
| Target | Corpus target name, such as `arrayvec-pr288`. |
| Card or family | Card id when available; otherwise the operation family or output cluster. |
| Primary label | One taxonomy label from this document. |
| Evidence | Short reason grounded in the dogfood output or manual inspection. |
| Follow-up | One concrete next action, or `none` when the observation is recorded only for context. |

## Reporting rules

- Prefer `needs-fixture` before `needs-analyzer` when the dogfood shape has no
  focused fixture yet.
- Use `noise` for reviewer-cost problems even if the card is technically
  defensible.
- Use `missed` only for a concrete inspected obligation, not for hypothetical
  unsupported breadth.
- Use `needs-route` when the operation is detected but the next credible review
  tool or human route is unclear.
- Use `needs-doc` when the product posture is right but the public explanation
  could overclaim, underexplain, or confuse the trust boundary.
- Use `needs-verifier` when the output is already correct but needs a checked
  rail to keep ReviewCard projections, receipts, badges, or policy summaries
  from drifting.
- Keep all dogfood artifacts advisory and local unless a report explicitly
  names a checked-in artifact.

## Trust boundary

Dogfood triage records selected reviewer-usefulness observations. It does not
prove memory safety, UB-free status, Miri-clean status, site execution,
calibrated precision, calibrated recall, witness adequacy, release readiness,
or policy readiness.
