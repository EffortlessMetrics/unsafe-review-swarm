# Dogfood report: 2026-06-13 post-fix card-correctness validation

Status: real-crate validation rerun
Swarm commit: `a496023c`
Artifact status: local, untracked under `target/dogfood-work/`

This report reruns the seven capped repo-snapshot targets in
[`corpus.toml`](../corpus.toml) against the analyzer after the 13
card-correctness fixes (#1672–#1684) landed, to confirm the fixes hold on real
code and to surface any regression or new finding. Each crate was scanned in
repo mode (`--max-cards 50`) and one agent reviewed a family-spanning sample of
its cards against the cited source.

## Trust boundary

It is not a support-tier promotion, calibration report, policy decision, safety
proof, UB-free claim, Miri-clean claim, witness result, site-execution proof, or
a calibrated precision/recall figure. No witness tools were run. The card counts
below are capped-scan samples, not a measured detection rate.

## Scope

Targets (pinned commits per `corpus.toml`), 50 cards each (capped):

- `smallvec-capped` — servo/rust-smallvec `bc8a8549`
- `arrayvec-capped` — bluss/arrayvec `1bc606d8`
- `memchr-capped` — BurntSushi/memchr `db1a77d4`
- `hashbrown-capped` — rust-lang/hashbrown `7b3bba6e`
- `bytes-capped` — tokio-rs/bytes `245adff0`
- `crossbeam-capped` — crossbeam-rs/crossbeam `03919fed`
- `mio-capped` — tokio-rs/mio `0d82f2a5`

Re-run any target with its `command` in `corpus.toml`. All seven runs exited 0
and reported `partial=true`, `stop_reason=max_cards` with the advisory
claim-boundary intact.

## Fixes validated (#1672–#1684)

All 13 fixes reproduced their corrected behavior on real code; none of the old
bugs recurred. Highlights:

- **Cross-obligation false discharge is gone (fixes #1/#2/#3).** Across the
  350 sampled cards, obligations are discharged only where genuinely justified.
  On `crossbeam` and `mio` exactly one obligation discharges across 50 cards;
  on `memchr` and `smallvec` the only discharges are sound (`MaybeUninit`
  element evidence). A capacity-only guard plus a stray `.len()` never credits
  `initialized` (`smallvec` set_len cards, `arrayvec` `remove`/`try_insert`); an
  unrelated `is_aligned`/`align_offset` never credits alignment (`hashbrown`
  `load_aligned` — the `debug_assert_eq!(ptr.align_offset(..),0)` correctly does
  NOT discharge); an unrelated `is_null`/`NonNull::new` never credits
  pointer-live.
- **FFI vs pure-Rust call (fix #4).** `smallvec` `.malloc_size_of()`,
  `arrayvec` `.extend_from_iter()`/`.encode_utf8()` are classed `unsafe_fn_call`,
  not `ffi_boundary`.
- **Reach word-boundary (fix #5).** Owner matches resolve to whole-word call
  sites; no longer-identifier substring hits observed.
- **Multiline transmute (fix #6).** `memchr`/`hashbrown` transmute cards keep a
  full snippet and both layout + valid-value obligations.
- **Projection consistency (#1679/#1683/#1684).** Every crate shows exactly 3
  `comment_plan_status=selected` cards (the `MAX_PLANNED_COMMENTS` budget),
  with `not_selected`/`not_eligible` distributed correctly and no `selected`
  leaking onto a `needs_human` card. `crossbeam` shows
  `agent_lsp_readiness=requires_witness_receipt` on witness-only cards
  (previously these projected `ready`).

### Per-crate verdict

| Crate | Confirmations | Regressions | New findings | Verdict |
|---|---:|---:|---:|---|
| smallvec | 7 | 0 | 0 | all sampled cards correct; only sound discharge is one `MaybeUninit` |
| arrayvec | 11 | 0 | 3 | fixes hold; one debug-assert bounds over-credit (stance); two multi-line `ptr::copy` snippet truncations |
| memchr | 5 | 0 | 3 | zero discharges across 50 cards; one `.add()` safe-method false positive |
| hashbrown | 7 | 0 | 2 | fixes hold; reach test-file over-classification |
| bytes | 8 | 0 | 3 | fixes hold; `zeroed` detector cards a safe `fn zeroed()` definition |
| crossbeam | 7 | 0 | 3 | one sound bounds discharge; `unsafe fn`-pointer struct field carded; reach noise |
| mio | 6 | 0 | 4 | fixes hold; 9 `.add()` safe-method false positives; `zeroed()` definition false positives |

## New findings (pre-existing analyzer issues real code exposed)

These are NOT caused by the 13 fixes; the fixture suite did not exercise them.
They are tracked in issue #1685; clearly-correct items are fixable without a
product decision (same anchoring class as #1672–#1684), stance items are owner
decisions.

### Clearly-correct false positives / misclassifications

- **`.add()` safe method read as pointer arithmetic** — the most material noise
  source. `Interest::READABLE.add(Interest::WRITABLE)` produces 9
  `pointer_arithmetic` cards in `mio`; `s.hash.add(byte)` (a safe newtype
  method) cards in `memchr`. The `.add(` substring matches regardless of
  receiver type (same anchoring class as the FFI method-receiver fix #4).
- **`zeroed` detector fires on safe `fn zeroed()` definitions** — `bytes`
  `pub fn zeroed(len)`, `mio` `io_status_block.rs` / `afd.rs` (the latter also
  mislocated to the signature line, not the `unsafe { zeroed() }` call).
- **`unsafe fn` pointer struct field carded as an operation** — `crossbeam`
  `deferred.rs:22` `call: unsafe fn(*mut u8),` is a type annotation, not an
  unsafe site.
- **Reach test-file over-classification** — `collect_test_files()` treats any
  `.rs` with `test` in its path or a `#[test]` anywhere as a "test file", so a
  production-only cross-reference (`hashbrown` `invalid_mut`, `into_inner`)
  counts as test reach.
- **`&mut *cell.get()` misclassed `unsafe_fn_call` instead of
  `raw_pointer_deref`** (`crossbeam` `sharded_lock.rs:197`).
- **Multi-line `ptr::copy(...)` snippet truncation** (`arrayvec` — same
  truncation class fix #6 addressed for transmute).

### Stance questions (owner decisions)

- Does a `debug_assert!`/`debug_assert_eq!` discharge a runtime bounds/alignment
  obligation? (Stripped in release; agents disagreed across `arrayvec`/`crossbeam`/`hashbrown`.)
- Reach noise on universal owner names (`drop`/`new`/`read`/`send`/`add`) — test
  files mention the name regardless of whether they exercise this site.
- Circular self-reach when an owner is a `#[test]` fn matching its own name.
- A conservative card on a trivially-sound `set_len(0)` (`clear()`).
- Block-opener anchoring losing a genuine FFI call inside the block (`memchr`
  libc bench).

## Outcome

The 13 card-correctness fixes are validated on real unsafe-heavy code with no
regressions and the trust boundary intact. Real-crate dogfooding surfaced a new
batch (dominated by the same substring-anchoring class) for a follow-up pass.
This rerun refreshes the post-fix posture of the seven capped snapshots; it does
not promote any claim to calibration or policy readiness.
