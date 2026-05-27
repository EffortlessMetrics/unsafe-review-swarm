# Dogfood report: 2026-05-27 hashbrown NonNull follow-up

Status: focused follow-up report
Swarm commit: `e16bf9b0`
Artifact status: local, untracked under `target/dogfood-work/`

This report follows the post-burst note that `NonNull::new_unchecked`
applicability had not been sampled in the selected dogfood run. The focused
`hashbrown-pr667` rerun exercises nested `NonNull::new_unchecked` operations in
iterator code and keeps them as concrete ReviewCards.

It is not a support-tier promotion, calibration report, policy decision, safety
proof, UB-free claim, Miri-clean claim, witness result, site-execution proof,
or NonNull precision claim. No witness tools were run.

## Scope

Target:

- `hashbrown-pr667`

Commands:

```bash
rtk proxy gh pr diff 667 -R rust-lang/hashbrown --patch > target/dogfood-work/hashbrown-pr667.raw.diff
rtk git -C target/dogfood-work/hashbrown fetch origin pull/667/head:dogfood-pr-667
rtk git -C target/dogfood-work/hashbrown worktree add --detach ../hashbrown-pr667-root dogfood-pr-667
rtk cargo run --locked -p unsafe-review -- check \
  --root target/dogfood-work/hashbrown-pr667-root \
  --diff target/dogfood-work/hashbrown-pr667.raw.diff \
  --format json \
  --max-cards 40 \
  --out target/dogfood-work/hashbrown-pr667.after-nonnull-applicability.json
```

Compared artifact:

```text
none; this is the first focused `hashbrown-pr667` NonNull follow-up artifact
```

## Summary

| Snapshot | Cards | Families | Classes | Reviewer note |
|---|---:|---|---|---|
| Current rerun | 4 | `nonnull_unchecked`, `unsafe_fn_call` | 4 `contract_missing` | The run produces two concrete `NonNull::new_unchecked` cards and two unsafe-call wrapper cards. |

## Current cards

| Operation family | Count | Classes | Reviewer note |
|---|---:|---|---|
| `nonnull_unchecked` | 2 | 2 `contract_missing` | `NonNull::new_unchecked(bucket.as_ptr())` remains a concrete nullability/contract review prompt. |
| `unsafe_fn_call` | 2 | 2 `contract_missing` | `self.raw.iter()` wrapper calls remain unsafe-call contract prompts. |

## NonNull evidence posture

The two `nonnull_unchecked` cards both report:

- contract evidence missing: no nearby `# Safety` docs or `SAFETY:` / `Safety:` comment detected;
- discharge evidence missing: no nullability guard code detected;
- static reach evidence present: related test files mention the owning methods;
- witness evidence missing: no imported witness receipt.

This is useful dogfood pressure for preserving concrete `nonnull_unchecked`
cards in nested iterator code. It is not a stale-pointer false-positive control;
future stale, wrong-pointer, macro, or cast/provenance controls still need
separate fixture or dogfood evidence.

## Triage observation

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `hashbrown-pr667` | `nonnull_unchecked` nested iterator operations | `actionable` | The rerun reports two concrete `NonNull::new_unchecked` cards with missing contract and nullability guard evidence. | Keep this as a regression target for concrete NonNull operation detection; add stale/wrong-pointer controls only from future fixture or dogfood pressure. |

## Trust boundary

This report records selected static ReviewCard output from a local dogfood run.
It does not prove the changed code safe, UB-free, Miri-clean, site-executed,
calibrated, or policy-ready. It does not establish NonNull precision or recall,
and it does not mean a witness ran.
