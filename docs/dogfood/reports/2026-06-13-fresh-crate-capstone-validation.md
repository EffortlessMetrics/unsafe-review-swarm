# Dogfood report: 2026-06-13 fresh-crate capstone validation

Status: fresh-crate capstone validation
Swarm commit: `4e388bc6`
Artifact status: local, untracked under `target/dogfood-work/`

This report runs the five fresh-crate capped repo-snapshot targets in
[`corpus.toml`](../corpus.toml) against the analyzer after all 24 card-correctness
fixes (#1672-#1699) landed, to confirm the fixes hold on real unsafe-heavy code
the analyzer had never seen before. Each crate was scanned in repo mode
(`--max-cards 50`). These crates were not part of the seven-crate wave used for
the prior fix passes.

## Trust boundary

It is not a support-tier promotion, calibration report, policy decision, safety
proof, UB-free claim, Miri-clean claim, witness result, site-execution proof, or
a calibrated precision or recall figure. No witness tools were run. The card counts
below are capped-scan samples, not a measured detection rate. This report is
advisory evidence only and does not constitute a policy readiness determination.

## Scope

Targets (pinned commits per `corpus.toml`), 50 cards each (capped):

- `bumpalo-capped` — fitzgen/bumpalo `300040d66d1fb35b96ba6529b6f1ff0e0ea075bc`
- `slab-capped` — tokio-rs/slab `a1e4346070a48c936d808de75191dee5d01e433c`
- `bytemuck-capped` — Lokathor/bytemuck `164cedda0eae131bc6cb67902599f4ec253642ca`
- `once_cell-capped` — matklad/once_cell `80fe900b21f6d76c1a2ed74d3343e8a3a88c46d0`
- `parking_lot-capped` — Amanieu/parking_lot `d20d71e5a8955ec1d2a53e3659142a505476bb3d`

All five runs exited 0 and produced consistent projections. Bumpalo, bytemuck,
once_cell, and parking_lot hit the 50-card cap (`partial=true`,
`stop_reason=max_cards`); slab produced 11 cards (no cap). Re-run any target
with its `command` in `corpus.toml`.

## Fixes validated (#1672-#1699)

All 24 fixes from the three prior passes reproduced their corrected behavior on
fresh code. The previously identified false-positive classes are absent:

- **No safe-context `.add()` pointer-arithmetic cards.** Bumpalo `ptr.add(next_read)`,
  slab `res_ptr.add(i)` are genuine in-unsafe pointer arithmetic (inside `unsafe {}`
  blocks). No safe-method `.add()` hits were observed.
- **No `fn zeroed(` or `fn assume_init(` definition cards.** The `maybe_uninit_assume_init`
  detector fires only on genuine `arc.assume_init()` call sites (bytemuck), not
  on function definitions.
- **No safe-constructor `from_raw_parts` cards.** Bumpalo
  `slice::from_raw_parts_mut(...)` and `String::from_utf8_unchecked(...)` cards
  are genuine in-unsafe operations.
- **Projection consistency holds.** Every crate shows exactly 3
  `comment_plan_status=selected` cards (the `MAX_PLANNED_COMMENTS` budget), with
  `not_selected`/`not_eligible` distributed correctly. `agent_lsp_readiness`
  correctly shows `requires_witness_receipt` alongside `ready`/`needs_human`
  (once_cell: 10 witness-receipt cards, parking_lot: 2) — the readiness-collapse
  fix (#1698) holds on fresh code.

### Per-crate summary

| Crate | Cards | Families observed | Partial? | Exit |
|---|---:|---|---|---|
| bumpalo | 50 | pointer_arithmetic, slice_from_raw_parts, str_from_utf8_unchecked, unsafe_fn_call, unsafe_impl | yes (max_cards) | 0 |
| slab | 11 | pointer_arithmetic, unsafe_fn_call, unsafe_impl | no | 0 |
| bytemuck | 50 | maybe_uninit_assume_init, unsafe_fn_call, unsafe_impl, raw_pointer_deref | yes (max_cards) | 0 |
| once_cell | 50 | unsafe_fn_call, raw_pointer_deref, unsafe_impl | yes (max_cards) | 0 |
| parking_lot | 50 | unsafe_fn_call, unsafe_impl, raw_pointer_deref, pointer_arithmetic | yes (max_cards) | 0 |

### Projection consistency notes

- `comment_plan_status=selected` count is exactly 3 per crate (MAX_PLANNED_COMMENTS budget).
- `agent_lsp_readiness=requires_witness_receipt`: once_cell 10, parking_lot 2; others 0.
- No `selected` leaking onto `needs_human` cards.
- `partial=true` with `stop_reason=max_cards` on capped runs; partial omitted on slab (11 cards, no cap).

## Residual noise

Legitimate `unsafe fn` declaration owner cards appear across all five crates (e.g.
`unsafe fn arith_offset(...)`, trait method signatures `unsafe fn prepare_park(&self);`).
These are the by-design owner-card-volume question tracked in issue #1671 and are
not regressions of the 24 fixes.

## Outcome

All 24 card-correctness fixes (#1672-#1699) are validated on fresh unsafe-heavy
code with no regressions and the trust boundary intact. The false-positive classes
eliminated in those fixes do not recur on any of the five fresh crates.
