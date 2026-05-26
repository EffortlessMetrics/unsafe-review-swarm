# Dogfood report: 2026-05-26 arrayvec Vec::set_len rerun

Status: focused rerun report
Swarm commit: `f66314b`
Artifact status: local, untracked under `target/dogfood-work/`

This report reruns `arrayvec-pr288` after the `Vec::set_len` applicability
refactors landed. It checks whether the `try_push_str` initialized-range
follow-up from the post-burst snapshot still needs a fixture before more
analyzer work.

It is not a support-tier promotion, calibration report, policy decision,
safety proof, UB-free claim, Miri-clean claim, witness result, or
site-execution proof. No witness tools were run.

## Scope

Target:

- `arrayvec-pr288`

Command:

```bash
rtk cargo run --locked -p unsafe-review -- check \
  --root target/dogfood-work/arrayvec \
  --diff target/dogfood-work/arrayvec-pr288.raw.diff \
  --format json \
  --max-cards 20 \
  --out target/dogfood-work/arrayvec-pr288.after-vec-set-len-applicability.json
```

Compared artifact:

```text
target/dogfood-work/arrayvec-pr288.after-encode-call-evidence.json
```

## Summary

| Snapshot | Cards | `vec_set_len` classes | Reviewer note |
|---|---:|---|---|
| Original post-burst | 8 | 5 `guarded_unwitnessed`, 1 `guard_missing`, 1 `unsafe_unreached` | One `try_push_str` card still looked like missing initialized-range evidence. |
| Current rerun | 8 | 6 `guarded_unwitnessed`, 1 `unsafe_unreached` | The `try_push_str` `set_len(new_len)` card now has guard evidence and remains witness-missing. |

## Current `Vec::set_len` cards

| Operation context | Operation | Class | Next action |
|---|---|---|---|
| `from_byte_string` | `vec.set_len(CAP);` | `guarded_unwitnessed` | Attach a focused Miri or cargo-careful witness receipt or mark the static limitation explicitly. |
| `try_push` | `self.set_len(len + n);` | `guarded_unwitnessed` | Attach a focused Miri or cargo-careful witness receipt or mark the static limitation explicitly. |
| `try_push_str` | `self.set_len(new_len);` | `guarded_unwitnessed` | Attach a focused Miri or cargo-careful witness receipt or mark the static limitation explicitly. |
| `pop` | `self.set_len(new_len);` | `guarded_unwitnessed` | Attach a focused Miri or cargo-careful witness receipt or mark the static limitation explicitly. |
| `truncate` | `self.set_len(new_len);` | `guarded_unwitnessed` | Attach a focused Miri or cargo-careful witness receipt or mark the static limitation explicitly. |
| `remove` | `self.set_len(new_len);` | `unsafe_unreached` | Add or identify a focused test path that reaches the safe wrapper around this unsafe seam. |
| `clear` | `self.set_len(0);` | `guarded_unwitnessed` | Attach a focused Miri or cargo-careful witness receipt or mark the static limitation explicitly. |

## Triage observation

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `arrayvec-pr288` | `Vec::set_len` initialized-range evidence | `actionable` | The rerun moves the `try_push_str` `set_len(new_len)` card from `guard_missing` to `guarded_unwitnessed`. | Keep the existing applicability refactors as regression pressure; add a fixture only if future dogfood exposes a new stale or wrong-target initialized-range shape. |

## Trust boundary

`guarded_unwitnessed` means static guard evidence was found and witness evidence
is still absent. It does not prove the initialized range is sound for all
callers, does not mean Miri or cargo-careful was run, and does not establish
memory-safety, UB-free, Miri-clean, site-execution, precision, recall, witness
adequacy, or policy readiness.
