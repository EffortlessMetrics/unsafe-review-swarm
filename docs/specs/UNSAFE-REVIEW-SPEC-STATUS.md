# Spec lifecycle status dashboard

This dashboard is an operator view over specification lifecycle state.

Until an automated generator lands, keep this file aligned with the current
active goal, implementation plan, and linked closeouts.

| Spec | Status | Implementation state | Proof commands | Last touched | Notes |
|---|---|---|---|---|---|
| `UNSAFE-REVIEW-SPEC-0019` first-run cockpit | accepted, release-prepped | Source and swarm lanes promoted; publication receipt documented | `cargo run --locked -p xtask -- check-first-pr-artifacts`; first-run smoke from release prep | 2026-05-21 | Reviewer-first cockpit contract for `doctor`, `first-pr`, `explain`, bundle honesty |
| `UNSAFE-REVIEW-SPEC-0020` source-of-truth stack | accepted, active-maintenance | Artifact taxonomy and linkage landed; current lane still tracks source-of-truth operations | `cargo run --locked -p xtask -- check-doc-artifacts`; `cargo run --locked -p xtask -- check-goals`; `cargo run --locked -p xtask -- check-ci-lanes`; `cargo run --locked -p xtask -- source-divergence` | 2026-05-21 | Requires active goal/plan freshness so the repo answers “what next?” without chat context |
| `UNSAFE-REVIEW-SPEC-0012` LSP/editor projection | accepted, partial-runtime | Saved projection contract promoted; live runtime remains limited to swarm lanes | `cargo run --locked -p xtask -- check-first-pr-artifacts`; projection contract/doc checks in `check-pr` | 2026-05-21 | Keep “no overclaim” boundary: saved projection is product truth; live server rollout is separate |
| `UNSAFE-REVIEW-SPEC-0023` first-hour experience | draft | First-hour rail added to bridge first-run cockpit output, explain/support usage, saved LSP, and bounded agent handoff | `cargo run --locked -p xtask -- check-docs`; `cargo run --locked -p xtask -- check-pr` | 2026-05-21 | Keeps post-first-run surfaces ReviewCard-derived and advisory |
| `UNSAFE-REVIEW-SPEC-0024` CI design | draft | CI lane taxonomy, permissions, example workflow shape, coverage posture, and source/swarm CI routing documented | `cargo run --locked -p xtask -- check-ci-lanes`; `cargo run --locked -p xtask -- check-pr`; `cargo run --locked -p xtask -- source-divergence` | 2026-05-23 | CI hard-fails malformed artifacts and repo policy failures, not advisory unsafe-review findings by default |
| `UNSAFE-REVIEW-SPEC-0021` VS Code/Open VSX extension | proposed | Planning rail only; extension client wiring is blocked until the SPEC-0018 live-LSP hardening gate is satisfied | `cargo run --locked -p xtask -- check-docs`; `cargo run --locked -p xtask -- check-goals`; `cargo run --locked -p xtask -- check-pr` | 2026-05-21 | Thin read-only adapter over `unsafe-review lsp`; no source edits, witness execution, telemetry by default, policy enforcement, or safety claim |
| `UNSAFE-REVIEW-SPEC-0025` docs automation | proposed | Policy ledger and `check-docs-automation` verifier landed; generator implementation remains follow-up | `cargo run --locked -p xtask -- check-docs-automation`; `cargo run --locked -p xtask -- check-doc-artifacts`; `cargo run --locked -p xtask -- check-goals`; `cargo run --locked -p xtask -- check-pr` | 2026-05-21 | Establishes machine-checkable docs automation control-plane boundaries before new generators land |
| `UNSAFE-REVIEW-SPEC-0026` accuracy validation and calibration | proposed | Label-ledger checker landed with fixture-pinned evidence, route-quality, and no-card honesty ledgers for raw pointer alignment/write initialization, slice::from_raw_parts_mut initialized-memory, public unsafe API docs, Box::from_raw ownership, ptr::drop_in_place Box-origin evidence/routes, generic unsafe call callee-contract evidence, get_unchecked_mut bounds evidence, pointer arithmetic bounds evidence, copy_nonoverlapping valid-range, ptr::copy valid-range, NonNull::new_unchecked nullability, MaybeUninit::assume_init, Vec::set_len, Vec::from_raw_parts capacity, transmute bool validity, UTF-8 unchecked conversion, `mem::zeroed` valid-zero evidence, `unreachable_unchecked` infallible-path evidence, unsafe impl Send/Sync, FFI route/obligation evidence, inline assembly human-review routing, static mut Loom/Shuttle routing, `Pin::new_unchecked` human-review routing, target-feature human-review routing, and zero-card controls | `cargo run --locked -p xtask -- check-calibration`; `cargo run --locked -p xtask -- check-dogfood`; `cargo run --locked -p xtask -- check-pr`; `cargo run --locked -p xtask -- source-divergence` | 2026-05-23 | Claim-scoped calibration only; no global precision/recall, no policy-ready claims |

## Reading notes

- **Status** is specification lifecycle intent (draft/accepted/etc.).
- **Implementation state** describes repository reality, including promotions and deferrals.
- **Proof commands** are the minimum commands that must stay green for the listed claim posture.

## Follow-up

Add/land an `xtask` dashboard check so this table becomes machine-validated rather
than purely editorial.
