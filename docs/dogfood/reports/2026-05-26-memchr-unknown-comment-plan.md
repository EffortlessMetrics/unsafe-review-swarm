# Dogfood report: 2026-05-26 memchr unknown comment-plan follow-up

Status: focused follow-up report
Swarm commit: `fcd3212`
Artifact status: no new dogfood artifact; comment-plan verifier and fixture smoke only

This report records the focused follow-up for the `memchr-capped` unknown
operation-family noise observation from the post-burst analyzer snapshot. The
original capped repository scan found many `unknown` unsafe owner cards. Those
cards are still useful as ReviewCards, but they are too broad for default inline
comment candidates.

The follow-up landed in swarm PR #479. It keeps `operation_family: "unknown"`
cards in the advisory bundle while marking them as not selected for
`comment-plan.json` with the reason:

```text
operation family unknown
```

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

Verifier and fixture-smoke evidence:

```bash
rtk cargo test -p unsafe-review-core comment_plan --locked
rtk cargo test -p unsafe-review --test e2e comment_plan --locked
rtk cargo test -p xtask advisory_artifact_checker --locked
rtk cargo run --locked -p unsafe-review -- first-pr --root fixtures/public_unsafe_fn_missing_safety --diff fixtures/public_unsafe_fn_missing_safety/change.diff --out-dir target/unsafe-review-comment-plan-unknown-family-smoke
rtk cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review-comment-plan-unknown-family-smoke
```

## Summary

| Follow-up | Result | Reviewer note |
|---|---|---|
| Unknown-family comment-plan selection | Comment-plan selector and artifact checker now reject inline comment candidates whose `operation_family` is `unknown`. | Broad unsafe-owner cards remain visible in the bundle, but the default inline-comment plan stays focused on specific, actionable operation families. |

## Triage observation

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `memchr-capped` | `unknown` unsafe-fn owner cards | `noise` | The original capped snapshot had twenty-four `unknown` cards, making it inventory-like rather than PR-review focused. | Swarm PR #479 keeps unknown-family cards out of inline comment candidates and records the not-selected reason as `operation family unknown`. |

## Trust boundary

This report records a projection and ranking invariant. It does not mean the
unknown cards are resolved, suppressed, safe, UB-free, Miri-clean, site-executed,
calibrated, or policy-ready. It does not mean a witness ran.
