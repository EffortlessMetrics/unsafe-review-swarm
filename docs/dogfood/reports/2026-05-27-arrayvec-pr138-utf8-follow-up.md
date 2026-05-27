# Dogfood report: 2026-05-27 arrayvec PR 138 UTF-8 follow-up

Status: focused follow-up report
Swarm commit: `b2519baa`
Artifact status: local, untracked under `target/dogfood-work/`

This report follows the post-burst note that UTF-8 unchecked validation was not
sampled and that `arrayvec-pr138` should be checked before treating it as that
coverage. The rerun shows that this target is useful, but it does not exercise
`str::from_utf8_unchecked`; it exercises `encode_utf8` unsafe function contracts,
raw pointer writes, pointer arithmetic, and one `Vec::set_len` card.

It is not a support-tier promotion, calibration report, policy decision, safety
proof, UB-free claim, Miri-clean claim, witness result, site-execution proof,
or UTF-8 unchecked validation claim. No witness tools were run.

## Scope

Target:

- `arrayvec-pr138`

Commands:

```bash
rtk proxy gh pr diff 138 -R bluss/arrayvec --patch > target/dogfood-work/arrayvec-pr138.raw.diff
rtk git -C target/dogfood-work/arrayvec fetch origin pull/138/head:dogfood-pr-138
rtk git -C target/dogfood-work/arrayvec worktree add --detach ../arrayvec-pr138-root dogfood-pr-138
rtk cargo run --locked -p unsafe-review -- check \
  --root target/dogfood-work/arrayvec-pr138-root \
  --diff target/dogfood-work/arrayvec-pr138.raw.diff \
  --format json \
  --max-cards 30 \
  --out target/dogfood-work/arrayvec-pr138.after-utf8-applicability.json
```

Compared artifact:

```text
none; this is the first focused `arrayvec-pr138` follow-up artifact
```

## Summary

| Snapshot | Cards | Families | Classes | Reviewer note |
|---|---:|---|---|---|
| Current rerun | 8 | `pointer_arithmetic`, `vec_set_len`, `unknown`, `raw_pointer_write`, `unsafe_fn_call` | 7 `contract_missing`, 1 `guarded_unwitnessed` | The target produces useful unsafe-function, raw-pointer, and `Vec::set_len` review prompts, but no `str_from_utf8_unchecked` ReviewCard. |

## Current cards

| Operation family | Count | Classes | Reviewer note |
|---|---:|---|---|
| `pointer_arithmetic` | 2 | 2 `contract_missing` | Pointer arithmetic around `ptr_mut().add(len)` and `ptr.add(index)` needs scoped contract evidence. |
| `vec_set_len` | 1 | 1 `contract_missing` | `self.set_len(len + n)` remains a concrete initialized-memory review prompt. |
| `raw_pointer_write` | 1 | 1 `contract_missing` | `ptr::write(ptr.add(index), byte)` needs pointer-validity evidence. |
| `unsafe_fn_call` | 2 | 2 `contract_missing` | Test calls to `encode_utf8` are unsafe-call review prompts, not UTF-8 unchecked validation evidence. |
| `unknown` | 2 | 1 `contract_missing`, 1 `guarded_unwitnessed` | The `write` and `encode_utf8` unsafe function owners remain broad owner cards. |

## Triage observation

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `arrayvec-pr138` | UTF-8 unchecked validation sampling | `needs-doc` | The focused run has no `str_from_utf8_unchecked` card, so it should not be cited as UTF-8 unchecked validation dogfood. | Keep fixture-backed `str::from_utf8_unchecked` applicability as the evidence source until a real dogfood target exercises that family. |
| `arrayvec-pr138` | `encode_utf8` unsafe function and raw pointer writes | `actionable` | The target still produces useful `unsafe_fn_call`, raw pointer, pointer arithmetic, and `Vec::set_len` cards. | Use this target for unsafe-function/raw-pointer wording checks, not for broadening UTF-8 unchecked recognizers. |

## Trust boundary

This report records selected static ReviewCard output from a local dogfood run.
It does not prove the changed code safe, UB-free, Miri-clean, site-executed,
calibrated, or policy-ready. It does not mean `arrayvec-pr138` exercises
`str::from_utf8_unchecked`, and it does not mean a witness ran.
