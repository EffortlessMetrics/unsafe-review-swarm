# Objective audit

Date: 2026-05-18
Status: active objective partially achieved; dogfood-calibrated evidence lane
closed experimentally; continue broader calibration before policy promotion

This audit maps the current product objective to concrete repo evidence. It is a
status artifact, not a support-tier promotion. `docs/status/SUPPORT_TIERS.md`
remains the authority for public claim wording.

Latest evidence-hardening notes include `get_unchecked` bounds false-positive
controls for bare predicate observations, closed positive branches, and checked
indexes reassigned before the unchecked access.
Raw pointer alignment evidence now also has fixture-backed controls for
same-pointer `is_aligned` guards, observations, closed branches, and stale
checked pointers.
Raw pointer bounds evidence now rejects bare predicate observations, closed
positive branches, post-use checks, and generic type angle brackets while
preserving the narrow same-slice `write_bytes` length shape.
Modulo alignment guards have the same fixture-backed observation, closed-branch,
and stale-pointer controls.
`Vec::from_raw_parts` len/cap capacity evidence now has fixture-backed controls
for assertions, early returns, bare observations, closed branches, and stale
checked cap arguments. `Vec::set_len` capacity evidence rejects unrelated local
arguments merely named `cap` unless a const-capacity context is visible.
`Box::from_raw` and `ptr::drop_in_place` ownership evidence reject stale
`Box::into_raw` origins when the raw pointer is reassigned before use.
Unchecked-constructor availability evidence now has fixture-backed controls for
same-receiver assertions, enclosing positive branches, unavailable-path early
returns, other receivers, bare observations, and closed branches.

The latest closed execution lane is recorded in
`docs/status/DOGFOOD_CALIBRATED_EVIDENCE_LANE.md` and
`docs/handoffs/2026-05-18-dogfood-calibrated-evidence-v0.6.md`.

## Objective

`unsafe-review` should be the cheap PR-time unsafe contract reviewer for Rust:
it identifies changed unsafe-adjacent seams, emits `ReviewCard`s, projects those
cards into review/editor/agent/repo surfaces, and routes reviewers to the
cheapest credible next witness without claiming soundness or running expensive
witnesses by default.

## Evidence Checklist

| Requirement | Current evidence | Status | Gap |
|---|---|---|---|
| Canonical product unit is `ReviewCard`; projections must not create parallel truth | JSON, PR summary, SARIF, comment-plan, saved LSP, agent packet, repo, badge, policy, and receipt surfaces are all listed in `SUPPORT_TIERS.md` as card projections; handoffs record lane boundaries | Experimental | Continue watching new surfaces for reclassification logic |
| Card correctness before breadth | Fixture goldens cover raw pointer alignment/deref/read/write including method-form volatile reads/writes and `write_bytes`, raw-pointer len/capacity equality bounds evidence, pointer arithmetic `num_ctrl_bytes` bounds evidence and same-slice end-pointer evidence, target-feature documented declaration contract evidence, split syntax, inline unsafe operation dedupe, attributed unsafe-fn dedupe, unsafe-call wrappers including multi-line wrappers, narrow `encode_utf8` remaining-capacity argument evidence, and unchecked-constructor availability evidence, multi-line `impl Trait` owner inference, nested unsafe operation parent-call dedupe, adjacent unchanged unsafe declaration filtering, unsafe contracts including documented public and private unsafe API declarations plus public `Safety:` doc prose and local `Safety:` comments, `MaybeUninit` `assume_init` / `assume_init_read` / `assume_init_ref` / `assume_init_mut` / `assume_init_drop`, `MaybeUninit` slice evidence, `Vec::set_len`, `Vec::set_len` initialized-loop evidence, `Vec::set_len` call-result initialization evidence, `Vec::set_len` shrink evidence, `Vec::set_len` last-index shrink evidence, `Vec::set_len` start-bound shrink evidence, `Vec::set_len(0)` clear evidence, `Vec::set_len` post-call initialization false-positive control, `Vec::from_raw_parts`, `Vec::from_raw_parts` same-pointer `ManuallyDrop` pointer/capacity, ownership, initialized-range, and len/capacity evidence, `Box::from_raw`, `copy_nonoverlapping`, overlapping `ptr::copy`, `ptr::replace`, `str::from_utf8_unchecked` including same-buffer `is_ok` enclosing branches, `is_err` early-return, question-mark propagation, match-return validation, post-validation, wrong-buffer, bare-observation, and stale-buffer false-positive-control fixtures, `mem::zeroed`, `static mut`, inline asm human-review routing, `transmute` including valid-value observation, closed-positive-branch, and stale-guard false-positive controls, `transmute_copy` including valid-value observation, closed-positive-branch, and stale-guard false-positive controls, multi-line `transmute_copy`, `unwrap_unchecked`, local infallible-result evidence plus same-receiver Option/Result state and if-let evidence for `unwrap_unchecked`, `unreachable_unchecked`, local infallible-path evidence plus other-context and post-evidence false-positive controls for `unreachable_unchecked`, `get_unchecked_mut` including same-receiver and post-check false-positive controls, `NonNull::new_unchecked` including bare-constructor observation, wrong-pointer, non-returning null branch, and post-check false-positive controls, `Pin::new_unchecked`, `drop_in_place`, `slice::from_raw_parts_mut`, FFI, unsafe impl Send/Sync including generic owner inference and generic-bound trait classification, and negative safe/comment cases; `fixtures/calibration.toml` indexes the core positive, negative, and false-positive-control claims | Experimental | Fixture corpus is curated; no broad semantic proof |
| Obligation-level evidence | `ReviewCard` output and fixture goldens distinguish contract, discharge, reach, and witness evidence per obligation | Experimental | Guard patterns remain sparse |
| Length guard does not discharge alignment; comments, operation names, and post-use checks do not count as guards | Raw-pointer alignment, comment-not-guard, and `NonNull::new_unchecked` fixture expectations are listed as proof in support tiers | Experimental | More real-world guard idioms need calibration |
| Stable-first implementation; no mandatory MIR or `rustc_private` | Workspace uses stable source parsing and `ra_ap_syntax`; support tiers mark MIR/nightly facts as deferred | Met for current lanes | Optional adapters still need ADR before promotion |
| Advisory PR artifact loop | Handoff `2026-05-18-advisory-pr-artifacts-v0.2.md` records cards JSON, PR summary, SARIF, and comment-plan artifact proof plus in-workflow artifact verification | Experimental/dogfoodable | No automatic comments or blocking policy by design |
| Saved IDE projection | Handoff `2026-05-18-lsp-agent-projection-v0.3.md` records `--format lsp` saved diagnostics, hovers, status data, copy-command data, and related-test open-command data | Experimental | No live LSP server or editor extension; static related-test mentions do not prove site execution |
| Bounded LLM packet | Handoff `2026-05-18-lsp-agent-projection-v0.3.md` records `context <card-id> --json` bounded packet proof | Experimental | Copy-only; no automated repair or source edits |
| Repo posture and badges count open review gaps, not raw unsafe or safety status | Handoff `2026-05-18-repo-policy-v0.4.md` and support tiers cover repo JSON, badge JSON, saved-snapshot outcome comparison, and first capped `memchr` outcome dogfood | Experimental | Not release-grade posture or calibrated governance |
| Baselines and suppressions use exact counted identity | Repo policy handoff records exact baseline/suppression matching and explicit no-new-debt mode | Experimental | Exact identity only; no broad suppressions; no calibrated blocking |
| Witness routing recommends cheap next action | Support tiers cover route-table tests and fixture routes for raw pointer, FFI, unsafe impl Send, Pin, invalid-value, and drop/deallocation cases | Experimental | Recommendation only unless a receipt is attached |
| Witness receipts attach external evidence without executing tools | Receipt docs and tests cover exact-card JSON import, metadata validation, tool/strength validation, DTO shape, template, validate command, Miri saved-output adapter, cargo-careful saved-output adapter, sanitizer saved-output adapter, Loom/Shuttle saved-output adapter, Kani/Crux proof saved-output adapter, and witness-plan output | Experimental | Saved-output adapters read success logs only; no witness tool is executed by `unsafe-review` |
| Explicit receipts can be authored and validated safely | `receipt template` and `receipt validate` are covered by CLI e2e tests and support tiers | Experimental | Template output does not verify that the recorded command ran |
| Public claims map to proof | `SUPPORT_TIERS.md` maps every current surface to proof and limits | In place | Keep updating for every new lane |
| No soundness, UB-free, Miri-clean, site-execution, or default-blocking claim | Trust-boundary text is enforced across artifacts; support tiers and handoffs repeat limits | In place | Must remain part of all new projections |
| First real-crate dogfood measurement | Handoff `2026-05-18-real-crate-dogfood-v0.6.md` records top-50 capped `rust-smallvec`, `arrayvec`, `memchr`, `hashbrown`, `bytes`, `crossbeam`, and `mio` runs plus `memchr#215`, `rust-smallvec#407`, `rust-smallvec#277`, `rust-smallvec#64`, `rust-smallvec#254`, `arrayvec#308`, `arrayvec#137`, `arrayvec#138`, `arrayvec#187`, `arrayvec#174`, `arrayvec#288`, `hashbrown#469`, `hashbrown#501`, `hashbrown#556`, `hashbrown#657`, `hashbrown#667`, `hashbrown#692`, `hashbrown#681`, `hashbrown#693`, `bytes#826`, `crossbeam#1226`, `crossbeam#1187`, and `mio#1388` PR-diff runs; dogfood found and fixed import/declaration false positives, adjacent unchanged unsafe declaration noise, `cfg(target_feature)` false positives, capped repo scan timeout behavior, syntax-scan performance on large changed files, missing owner-contract inheritance for operation cards, comment-derived owner false positives, multi-line `impl Trait` owner false positives, generic unsafe impl owner and Send/Sync trait false positives, `Safety:` doc and local comment contract evidence gaps, attributed unsafe-fn duplicates, inline unsafe-block duplicates, `drop_in_place` operation modeling from `arrayvec#174`, documented public unsafe API declaration handling and unsafe-call wrapper labeling plus remaining-capacity argument evidence from `arrayvec#288`, documented private unsafe declaration handling, `slice::from_raw_parts_mut` operation modeling and `MaybeUninit` slice evidence from `hashbrown#692`, `write_bytes` raw pointer write modeling and `MaybeUninit` raw-write destination evidence, `num_ctrl_bytes` and same-slice end-pointer arithmetic bounds evidence, target-feature declaration contract evidence from the capped `memchr` rerun, len/capacity equality bounds evidence for raw pointer reads from `arrayvec#187`, and `&'static mut` false-positive control from `hashbrown#692`, `unwrap_unchecked` invalid-value operation modeling and local infallible-result evidence from `hashbrown#693`, `unreachable_unchecked` unreachable-path operation modeling and local infallible-path evidence from `hashbrown#469`, multi-line unsafe-call wrapper labeling from `hashbrown#657`, unsafe-call contract and raw-pointer deref measurement from `hashbrown#681`, unchecked-constructor availability evidence from the capped `memchr` repo rerun, parent-call dedupe for nested `NonNull::new_unchecked` operations from `hashbrown#667`, fixture-backed `Vec::from_raw_parts` operation modeling from `bytes#826`, and fixture-backed `Vec::set_len` evidence improvements with `arrayvec#288`, `rust-smallvec#277`, and `rust-smallvec#64` reruns, including call-result initialization evidence; `arrayvec#137` adds raw pointer accessor soundness-fix measurement, capped `crossbeam` dogfood adds concurrency-heavy Send/Sync, atomic-ordering, raw pointer, ownership-transfer, and transmute_copy cards including multi-line call snippets, `crossbeam#1226` adds strict-provenance Miri cfg atomic pointer contract measurements, `crossbeam#1187` adds atomic pointer state transition measurement, and `mio#1388` adds socket-address layout conversion measurement with zeroed values, raw pointer writes, raw pointer dereferences, and unsafe function call contracts | Experimental | More crates, more real PR diffs, uncapped/sampled runs, broader `Vec::set_len`, `Vec::from_raw_parts` allocator/layout evidence, contract evidence, owner inference beyond the covered generic unsafe impl shapes, unsafe-call, mutable slice, raw pointer write byte-pattern validity, pointer-arithmetic guard naming beyond narrow checked shapes, target-feature availability proof beyond documented declarations, option/result state proof inference beyond local infallible-result evidence, control-flow reachability proof beyond local infallible-path evidence, nested operation attribution, large-repo performance calibration beyond this hashbrown fix, drop/deallocation evidence modeling, atomic pointer state modeling beyond narrow null swaps, Send/Sync invariant evidence beyond route selection, transmute_copy value-validity proof, and human review are still needed before calibration claims |

## Current Gaps

These are not failures; they are the next unsupported or weakly verified areas:

- Live LSP server and editor extension remain planned.
- The first saved-output adapters only import saved Miri, cargo-careful, sanitizer,
  Loom, Shuttle, Kani, and Crux success logs.
- Witness tools are not executed by `unsafe-review`, and no lane should add
  default execution without a separate plan.
- Schema compatibility is not yet a public promise.
- Broader calibration on real unsafe-heavy crates is still needed before any
  support tier promotion toward usable alpha. The first dogfood slice covered
  seven top-50 capped repo snapshots and twenty-three PR diffs across seven crates;
  the fixture calibration manifest remains a proof index, not real-world
  calibration.
- No default no-new-debt or blocking branch-protection policy is justified yet.
- Outcome comparison is saved-snapshot only. It now has first capped `memchr`
  repo snapshot dogfood, but needs more repos and PR snapshot pairs before
  dashboard-like posture claims.
- `crossbeam#1187` now has fixture and dogfood-rerun coverage for the narrow
  `swap(ptr::null_mut(), Ordering::...)` atomic pointer state transition shape,
  but broader safe-looking atomic pointer state changes that affect
  drop/deallocation invariants remain unsupported semantic work.
- Real PR-diff dogfood shows `Vec::set_len` guard evidence still needs broader
  modeling; visible `MaybeUninit::new` initialization loops and const `CAP`
  capacity facts now have fixture coverage, same-vector
  `Vec::with_capacity(new_len)` capacity evidence has fixture coverage, and
  unrelated capacity comparisons plus local arguments merely named `cap` are
  pinned as non-evidence before `set_len`;
  `arrayvec#288` has a rerun receipt;
  non-zero shrink and `set_len(0)` clear evidence also have fixture and
  dogfood-rerun coverage, start-bound shrink evidence has fixture and
  `rust-smallvec#277` dogfood-rerun coverage, and last-index shrink evidence
  has fixture and `rust-smallvec#64` dogfood-rerun coverage, while other
  `set_len` patterns remain weak.
- Real PR-diff dogfood now recognizes `ptr::drop_in_place` as a
  drop/deallocation operation family, and fixture coverage recognizes the
  narrow same-pointer `Box::into_raw` origin shape as drop evidence while
  rejecting reassigned raw pointers, but broader drop/deallocation evidence
  modeling remains narrow.
- `arrayvec#137` adds a raw pointer accessor soundness-fix measurement. It
  produced 15 contract-missing cards when run with a PR-head checkout and raw
  `diff --git` patch, which is useful dogfood but not calibration proof.
- Real PR-diff dogfood now recognizes `slice::from_raw_parts_mut` as the
  `slice_from_raw_parts` operation family, but broader mutable-slice range proof
  remains source-level and advisory.
- Real PR-diff dogfood now recognizes `MaybeUninit` slice element context as
  initialized-memory evidence for `slice::from_raw_parts_mut`, but it does not
  discharge pointer validity, alignment, allocation, or witness evidence.
- Real PR-diff dogfood now recognizes raw pointer `write_bytes` as a
  `raw_pointer_write` operation family. Fixture coverage also recognizes the
  narrow `*mut u8` case as alignment and byte-pattern evidence and recognizes
  `MaybeUninit` raw-write destinations as initialized-memory evidence. Other
  destination-type modeling remains source-level and advisory. The narrow
  same-slice `slice.as_mut_ptr().write_bytes(_, slice.len())` shape can
  discharge bounds evidence, while write-side fixture coverage now proves bare
  length observations, closed positive branches, and post-use checks do not
  discharge bounds. Generic type angle brackets also do not discharge raw
  pointer bounds evidence. Method-form raw pointer writes now have fixture proof
  for same-receiver alignment guards and stale observed, closed-branch, and
  post-write alignment checks. Method-form raw pointer reads, writes,
  `read_unaligned`, and `write_unaligned` now also require same-receiver,
  pre-operation nullability guards for pointer-live evidence and reject observed
  nullability, other-pointer guards, and post-use checks.
  These raw-pointer evidence rules still do not discharge
  pointer validity, allocation, or witness obligations.
- Real PR-diff dogfood now recognizes `index < self.num_ctrl_bytes()` as bounds
  evidence for pointer arithmetic, and capped `memchr` repo dogfood recognizes
  the local same-slice `as_ptr()` plus `len()` end-pointer pattern, but broader
  pointer-arithmetic guard naming remains uncalibrated.
- Real PR-diff dogfood now recognizes `len == capacity` and `assert_eq!` /
  `debug_assert_eq!` len/capacity checks as bounds evidence for raw pointer
  reads, but it does not infer alignment, initialization, or same-allocation
  proof from those checks.
- Public unsafe API declarations with recognized `# Safety` or doc-comment
  `Safety:` docs no longer ask for local declaration guards, but static reach
  remains a heuristic name search.
- Private unsafe declarations with recognized `# Safety` docs no longer ask for
  local declaration guards, but unsafe-call-specific callee contract inference
  remains future work.
- The `arrayvec#288` `set_len(len + n)` call-result pattern now has fixture and
  dogfood-rerun coverage, but unsafe-call-specific modeling for the surrounding
  `encode_utf8` wrapper remains future work.
- The `arrayvec#288` `encode_utf8` wrapper is labeled as `unsafe_fn_call` and
  the narrow remaining-capacity argument shape is recognized, but broader
  callee-specific safety contract inference remains future work.
- The capped `memchr` repo rerun no longer treats arbitrary `new_unchecked`
  constructors as `nonnull_unchecked`, and visible `is_available()` wrappers can
  discharge unsafe-call callee-contract evidence, but deeper callee-specific
  target-feature modeling remains future work.
- The capped `memchr` repo rerun now treats documented target-feature
  declarations as caller-contract sites instead of local-guard sites, but this
  is contract evidence only and does not prove target-feature availability.
- The `hashbrown#693` `unwrap_unchecked` sites are labeled as invalid-value
  operation cards, and local `Fallibility::Infallible`, same-receiver
  enclosing `is_some` / `is_ok` branches, early-return, and narrow
  `if let ... as_ref()` state evidence is recognized for `unwrap_unchecked()`;
  bare state observations and stale receiver-state evidence after reassignment
  remain guard-missing false-positive controls. Broader option/result state
  proof inference remains future work.
- The `hashbrown#469` `unreachable_unchecked` sites are labeled as
  invalid-value operation cards, and local `Fallibility::Infallible` error-path
  evidence is recognized while other-context, post-evidence, and closed-match
  fixtures keep false positives pinned. Broader control-flow reachability proof
  inference remains future work.
- The `hashbrown#469` multi-line `impl Trait` parameter owners now resolve to
  enclosing function names instead of `Fn`, but deeper callee-contract inference
  remains future work.
- The `hashbrown#657` multi-line unsafe call wrappers are labeled as
  `unsafe_fn_call`, but callee-specific contract inference and precise call-path
  extraction remain future work.
- The `hashbrown#667` nested `NonNull::new_unchecked` parent-call duplicate is
  removed, but broader nested operation attribution remains heuristic.
- The `hashbrown#501` adjacent unchanged unsafe declaration card is removed, but
  fallback declaration range handling is intentionally stricter than operation
  neighborhood matching.
- Syntax scanning on large `hashbrown` changed files now avoids whole-source
  line/column rescans per syntax node and skips impossible syntax node kinds
  before snippet normalization, but broader large-repo performance calibration
  still needs more repos and uncapped or sampled runs.
- `str::from_utf8_unchecked` now recognizes same-buffer `is_ok`, `is_err`
  early-return, question-mark propagation, and match-return validation evidence, and has
  post-validation and wrong-buffer false-positive-control fixtures. Broader
  indirect wrapper validation and aliasing-sensitive byte-slice equivalence
  remain unsupported.
- The `bytes#826` `Vec::from_raw_parts` site is now labeled as a Vec ownership
  operation rather than a slice operation, and fixture coverage recognizes the
  narrow same-pointer `ManuallyDrop` raw-parts origin shape as pointer/capacity,
  ownership, initialized-range, and len/capacity evidence. Broader allocator
  compatibility, layout, and ownership evidence remain source-level and
  advisory.
- `Box::from_raw` now has fixture coverage for the narrow same-pointer
  `Box::into_raw` origin shape and rejects reassigned raw pointers, but broader
  allocator and unique-ownership evidence remains source-level and advisory.

## Current Gates

Use these commands for a broad local proof pass:

```bash
rtk cargo fmt --check
rtk cargo check --workspace --all-targets --locked
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --locked
rtk cargo run --locked -p xtask -- check-pr
rtk cargo run --locked -p xtask -- check-calibration
rtk git diff --check
```

Targeted proof commands added by recent receipt work:

```bash
rtk cargo test -p unsafe-review-core receipt --locked
rtk cargo test -p unsafe-review-core imported_receipt --locked
rtk cargo test -p unsafe-review-cli receipt_template --locked
rtk cargo test -p unsafe-review-cli receipt_validate --locked
rtk cargo test -p unsafe-review --test e2e receipt_template --locked
rtk cargo test -p unsafe-review --test e2e receipt_validate --locked
```

## Recommended Next Lane

Continue dogfood measurement before policy promotion:

1. Run `unsafe-review` on more selected real unsafe-heavy crates and record
   false-positive and false-negative notes.
2. Measure card usefulness on more real PR diffs, not only repo snapshots.
3. Dogfood explicit receipts and outcome comparison on more real unsafe-review
   PRs.
4. Preserve exact-card matching, visible limitations, and advisory-only policy.
5. Keep support tiers experimental until dogfood and calibration justify a
   stronger claim.
