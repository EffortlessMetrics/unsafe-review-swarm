# Dogfood report: 2026-05-28 memchr target-feature posture

Status: focused follow-up report
Swarm commit: `4e9099cd`
Artifact status: no new dogfood artifact; fixture and comment-plan smoke only

This report records the focused follow-up for the `memchr-capped`
`target_feature` observation from the post-burst analyzer snapshot. The original
capped repository scan showed ten `target_feature` cards as
`guarded_unwitnessed`, which is the intended posture: target-feature
documentation can be useful caller-contract evidence, but it is not hardware
availability proof, dispatch proof, site-execution proof, a Miri result, or a
reason to broaden analyzer claims.

It is not a support-tier promotion, calibration report, dogfood rerun, policy
decision, safety proof, UB-free claim, Miri-clean claim, witness result, or
site-execution proof. No witness tools were run.

## Scope

Target:

- `memchr-capped`

Original dogfood artifact:

```text
target/dogfood-work/memchr.unsafe-review.after-target-feature-contract-evidence.json
```

Fixture and projection smoke:

```bash
rtk cargo run --locked -p unsafe-review -- check \
  --root fixtures/target_feature_safety_docs \
  --diff change.diff \
  --format json \
  --out target/target-feature.json

rtk cargo run --locked -p unsafe-review -- check \
  --root fixtures/target_feature_safety_docs \
  --diff change.diff \
  --format comment-plan \
  --out target/target-feature-comment-plan.json
```

## Summary

| Follow-up | Result | Reviewer note |
|---|---|---|
| Target-feature contract posture | Fixture-smoke confirmed | `target_feature_safety_docs` remains `guarded_unwitnessed`: contract and discharge evidence are present, witness evidence is still absent, and the route stays `human-deep-review`. |
| Inline comment budget | Comment-plan smoke confirmed | The target-feature card is not selected for inline comments by default because it is medium priority / medium confidence; it remains visible in `not_selected[]` with ReviewCard context. |

## Triage observation

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `memchr-capped` | `target_feature` cards | `actionable` | The post-burst snapshot showed target-feature cards preserving contract evidence while leaving witness evidence absent. | Keep `target_feature_safety_docs` as the fixture guardrail and keep target-feature cards out of availability, site-execution, or Miri-result wording. |
| `target_feature_safety_docs` | comment-plan projection | `needs-verifier` | The fixture projection accounts for the card in `not_selected[]` rather than inline comments, preserving review budget while keeping the card visible. | Continue relying on `check-first-pr-artifacts` / `check-pr` rails for comment-plan accounting; add a new verifier only if a future target-feature card drifts into overclaiming wording. |

## Trust boundary

This report records target-feature review posture from existing dogfood and
fixture smoke. It does not mean hardware availability was checked, a dispatch
path executed, Miri or a witness ran, or the code is safe. It is not UB-free
status, not Miri-clean status, not site-execution proof, not calibrated
precision/recall, and not policy-ready.
