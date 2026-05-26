# Dogfood report: 2026-05-26 crossbeam atomic pointer rerun

Status: focused rerun report
Swarm commit: `b481972`
Artifact status: local, untracked under `target/dogfood-work/`

This report reruns `crossbeam-pr1226` after the atomic pointer/state classifier
and fixtures landed. It checks whether the original post-burst `unknown`
cards still need analyzer breadth.

It is not a support-tier promotion, calibration report, policy decision,
safety proof, UB-free claim, Miri-clean claim, Loom/Shuttle result, or
site-execution proof. No witness tools were run.

## Scope

Target:

- `crossbeam-pr1226`

Command:

```bash
rtk cargo run --locked -p unsafe-review -- check \
  --root target/dogfood-work/crossbeam-pr1226-root \
  --diff target/dogfood-work/crossbeam-pr1226.raw.diff \
  --format json \
  --max-cards 40 \
  --out target/dogfood-work/crossbeam-pr1226.after-atomic-pointer-state.json
```

Compared artifact:

```text
target/dogfood-work/crossbeam-pr1226.strict-provenance.head.json
```

## Summary

| Snapshot | Cards | Families | Classes | Reviewer note |
|---|---:|---|---|---|
| Original post-burst | 6 | `unknown` | 6 `contract_missing` | Too broad for the changed atomic pointer/state operations. |
| Current rerun | 6 | `atomic_pointer_state` | 6 `requires_loom` | The cards now name the atomic pointer/state operation family and route review toward Loom/Shuttle or human concurrency review. |

## Current cards

| Operation shape | Count | Class | Next action |
|---|---:|---|---|
| `Shared::from_ptr(...fetch_and...)` | 2 | `requires_loom` | Add or update a Loom/Shuttle model for the changed concurrency invariant. |
| `Shared::from_ptr(...fetch_or...)` | 2 | `requires_loom` | Add or update a Loom/Shuttle model for the changed concurrency invariant. |
| `Shared::from_ptr(...fetch_xor...)` | 2 | `requires_loom` | Add or update a Loom/Shuttle model for the changed concurrency invariant. |

## Triage observation

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `crossbeam-pr1226` | `atomic_pointer_state` fetch operations | `actionable` | The rerun classifies all six changed atomic pointer/state operations as `atomic_pointer_state` and routes them to `requires_loom`. | Keep the existing fixtures as regression coverage; add another fixture only if future dogfood exposes a missing atomic pointer/state shape. |

## Trust boundary

These cards route review toward concurrency modeling. They do not mean Loom or
Shuttle was run, do not prove the changed code safe, and do not establish
memory-safety, UB-free, Miri-clean, site-execution, precision, recall, witness
adequacy, or policy readiness.
