# Dogfood report: 2026-05-29 arrayvec Self::new capacity control

Status: focused dogfood-backed fixture control
Swarm commit: `61305f7c`
Artifact status: local, untracked under `target/dogfood-work/`

This report reruns the `arrayvec-pr288` target after the first-pr cockpit and
agent repair-queue projection work. The goal is to record one concrete
`Vec::set_len` capacity evidence shape: initialization evidence can be visible
for `vec.set_len(CAP)`, while capacity evidence remains missing when the
fixed-capacity storage is hidden behind an opaque `Self::new()` constructor.

It is not a support-tier promotion, calibration report, policy decision,
safety proof, UB-free claim, Miri-clean claim, witness result, or
site-execution proof. No witness tools were run.

## Scope

Target:

- `arrayvec-pr288`

Commands:

```bash
rtk proxy gh pr diff 288 -R bluss/arrayvec --patch \
  > target/dogfood-work/arrayvec-pr288.raw.diff
rtk git clone https://github.com/bluss/arrayvec target/dogfood-work/arrayvec
rtk git -C target/dogfood-work/arrayvec fetch origin pull/288/head:dogfood-pr-288
rtk git -C target/dogfood-work/arrayvec checkout --detach dogfood-pr-288
rtk cargo run --locked -p unsafe-review -- first-pr \
  --root target/dogfood-work/arrayvec \
  --diff target/dogfood-work/arrayvec-pr288.raw.diff \
  --out-dir target/dogfood-work/arrayvec-pr288.2026-05-29.first-pr-smoke
```

Artifact verification:

```bash
rtk cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/dogfood-work/arrayvec-pr288.2026-05-29.first-pr-smoke
```

## Summary

| Surface | Result | Reviewer note |
|---|---:|---|
| `cards.json` | 8 cards, 8 open actionable gaps | 1 `guard_missing`, 6 `guarded_unwitnessed`, 1 `unsafe_unreached`. |
| `pr-summary.md` | Verified | The reviewer cockpit selects the `from_byte_string` `vec.set_len(CAP)` card as the top card and names missing guard evidence. |
| `github-summary.md` | Verified | The bounded doorway remains artifact-oriented and does not duplicate the full card table. |
| `comment-plan.json` | Verified | Planned comments stay bounded and card-derived. |
| `witness-plan.md` | Verified | The `vec_set_len` cards stay routed to advisory Miri/cargo-careful witness routes. |
| `lsp.json` | Verified | Saved editor projection remains read-only and card-derived. |

## Top card

```text
ID: UR-arrayvec-src-array-string-rs-from-byte-string-operation-vec_set_len-set-len-073a0fa631f6-initialized_memory-c1
Class: guard_missing
Location: src/array_string.rs:140
Operation: vec.set_len(CAP);
Operation family: vec_set_len
Missing evidence: Missing visible local guard for inferred safety obligations; No witness receipt imported for this card
Primary route: miri
Next action: Add or expose the local guard that discharges the `vec_set_len` safety obligation.
```

The card has present initialization evidence for the initialized-range
obligation, but the capacity obligation remains missing. Manual inspection
shows why this is conservative: the loop writes through `out.xs`, but the
capacity relation for `set_len(CAP)` is only implicit through `Self::new()` and
the type-level `CAP`. This dogfood shape should not silently clear capacity
evidence unless a later PR deliberately supports opaque constructor capacity
evidence with focused controls.

## Fixture follow-up

The checked fixture `vec_set_len_self_new_const_cap_not_guard` distills this
shape:

- same receiver initialized through a visible loop;
- `set_len(CAP)` on the same value;
- storage capacity hidden behind an opaque `Self::new()` constructor;
- expected card class remains `guard_missing`;
- initialized evidence is present, but capacity discharge is missing.

This is a false-positive control against over-broad capacity inference, not a
new recognizer.

## Triage observation

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `arrayvec-pr288` | `Vec::set_len` capacity evidence through `Self::new()` | `needs-fixture` | The rerun shows visible initialization evidence but missing capacity evidence for `vec.set_len(CAP)` when capacity is implicit behind `Self::new()`. | Keep the fixture `vec_set_len_self_new_const_cap_not_guard` as regression pressure; do not broaden constructor-capacity inference without a separate fixture-backed PR. |

## Trust boundary

This is static unsafe contract review dogfood. It does not prove memory safety,
UB-free status, Miri-clean status, unsafe-site execution, witness adequacy,
release readiness, or policy readiness. It is not calibrated precision or
calibrated recall evidence. `unsafe-review` did not run witnesses, post
comments, edit source, or enforce blocking policy.
