# Dogfood Usefulness Notes

Date: 2026-05-21
Status: experimental selected-corpus notes
Source manifest: [`corpus.toml`](corpus.toml)

These notes explain what the selected dogfood targets are useful for when
calibrating `unsafe-review`. They are not benchmark results, precision/recall
claims, release readiness proof, or safety evidence for the upstream projects.

Dogfood answers a narrower question:

```text
Does this ReviewCard shape help a reviewer ask for the next contract, guard,
test, or witness?
```

## Trust Boundary

Dogfood records are static unsafe contract review evidence. They do not prove
memory safety, UB-free status, Miri-clean status, site execution, or policy
readiness. Saved artifacts are local and untracked under `target/dogfood-work/`
unless a handoff explicitly says otherwise.

## How To Read A Dogfood Target

Each target should have a narrow reason to exist:

- **Hazard family:** the operation or review pattern the target exercises.
- **Useful signal:** what the target can tell us about card quality.
- **Remaining noise:** the limitation or false-positive risk to watch.
- **Fixture backing:** local fixtures that keep the rule honest.
- **Support impact:** whether this target changed support-tier wording.

When a target has no recorded before/after outcome, treat it as selected corpus
coverage only. It is not evidence that a rule improved.

## Repository Notes

| Repository | Useful signal | Remaining noise / limit | Fixture backing | Support impact |
|---|---|---|---|---|
| `servo/rust-smallvec` | Exercises raw pointer access, `Vec::set_len`, pointer arithmetic, and unsafe impl cards in compact unsafe-heavy code. | Pointer aliasing and ownership evidence can be deeper than local syntax; cards should stay advisory and operation-specific. | `raw_pointer_alignment`, `raw_pointer_write_*`, `vec_set_len_*`, `unsafe_impl_send` | No calibrated promotion; corpus evidence only. |
| `bluss/arrayvec` | Exercises `MaybeUninit`, `Vec::set_len`, raw pointer writes, UTF-8 unchecked conversion, and drop/deallocation cards. | Soundness-fix PRs may intentionally move unsafe patterns instead of simply adding guards; outcome review must preserve that context. | `maybeuninit_assume_init*`, `vec_set_len_*`, `str_from_utf8_unchecked*`, `drop_in_place_*` | No calibrated promotion; corpus evidence only. |
| `BurntSushi/memchr` | Exercises SIMD target-feature contracts, pointer arithmetic, and unchecked constructor cards. | Target-feature docs are contract evidence, not runtime target-feature availability or site execution. | `target_feature_safety_docs`, `pointer_arithmetic_*`, `unchecked_constructor_*` | One recorded outcome moved target-feature cards from `guard_missing` to `guarded_unwitnessed` without stronger safety claims. |
| `rust-lang/hashbrown` | Exercises large-file scanning, `MaybeUninit`, pointer arithmetic, unchecked/infallible operations, unsafe-call contracts, and dedupe behavior. | Large diffs can expose nearby unchanged unsafe declarations; card identity and changed-range filtering must stay tight. | `maybeuninit_assume_init*`, `unreachable_unchecked_*`, `unwrap_unchecked_*`, `adjacent_unchanged_unsafe_fn_no_card` | No calibrated promotion; corpus evidence only. |
| `tokio-rs/bytes` | Exercises `Vec::from_raw_parts`, slice construction, and ownership-transfer obligations. | Allocation provenance and ownership transfer often need human review beyond local syntax. | `vec_from_raw_parts*`, `slice_from_raw_parts_mut*`, `box_from_raw*` | No calibrated promotion; corpus evidence only. |
| `crossbeam-rs/crossbeam` | Exercises unsafe Send/Sync, atomics, strict-provenance cfg cards, raw pointers, and atomic pointer state transitions. | Interleavings and atomics are not Miri-lite; routes should point to Loom/Shuttle or human review when appropriate. | `unsafe_impl_send*`, `unsafe_impl_sync_generic_bound`, `atomic_pointer_state_swap` | No calibrated promotion; corpus evidence only. |
| `tokio-rs/mio` | Exercises unsafe function call contracts, `Vec::set_len`, zeroed values, socket-address layout conversions, and unsafe Send/Sync route cards. | Layout and platform contracts may need sanitizer, proof-tool, or human review rather than local guard evidence. | `unsafe_fn_call_*`, `vec_set_len_*`, `zeroed_*`, `transmute_*`, `unsafe_impl_send*` | No calibrated promotion; corpus evidence only. |

## Recorded Outcome Notes

### `memchr-capped` target-feature contract evidence

Recorded movement:

```text
new: 0
resolved: 0
improved: 10
regressed: 0
unchanged: 40
```

What improved:

- Documented `#[target_feature]` declarations moved from `guard_missing` to
  `guarded_unwitnessed`.
- The rule now treats local target-feature documentation as contract evidence.

What did not change:

- No target-feature availability is proven.
- No unsafe site execution is proven.
- No soundness, UB-free, or Miri-clean claim is made.
- Witness evidence still requires an imported receipt.

Useful follow-up:

- Keep `target_feature_safety_docs` and `cfg_target_feature_not_operation` as
  fixtures that distinguish target-feature declarations from cfg predicates.
- Dogfood more target-feature cards only when the output shows whether a
  reviewer can take a concrete next action.

## Target Notes

| Target | Hazard family | Useful signal | Remaining noise / limit | Fixture backing |
|---|---|---|---|---|
| `smallvec-capped` | Raw pointer, `Vec::set_len`, pointer arithmetic, unsafe impls | Broad capped snapshot for compact unsafe abstractions. | Capped output is inventory-like; review exact PR diffs before changing support claims. | `raw_pointer_alignment`, `vec_set_len_*`, `unsafe_impl_send` |
| `arrayvec-capped` | `MaybeUninit`, `Vec::set_len`, raw pointer, UTF-8, drop/deallocation | Broad snapshot for initialization and capacity evidence. | Soundness-fix code may intentionally reduce safe abstractions; avoid simplistic "unsafe count" readings. | `maybeuninit_assume_init*`, `vec_set_len_*`, `str_from_utf8_unchecked*` |
| `memchr-capped` | SIMD target features, unchecked constructors, pointer arithmetic | Outcome proof for target-feature contract evidence. | Still no target-feature availability or site-execution proof. | `target_feature_safety_docs`, `pointer_arithmetic_*` |
| `hashbrown-capped` | Large-file scan, `MaybeUninit`, unchecked operations | Stress target for card identity and syntax-scan behavior. | Nearby unchanged unsafe declarations can be noisy if changed-range handling regresses. | `adjacent_unchanged_unsafe_fn_no_card`, `maybeuninit_assume_init*` |
| `bytes-capped` | `Vec::from_raw_parts`, slices, ownership transfer | Ownership-transfer card quality. | Provenance and allocator ownership can exceed local static evidence. | `vec_from_raw_parts*`, `slice_from_raw_parts_mut*` |
| `crossbeam-capped` | Unsafe Send/Sync, atomics, raw pointers | Concurrency route quality and atomic pointer state cards. | Interleaving evidence needs Loom/Shuttle or human review, not a local static proof. | `unsafe_impl_send*`, `atomic_pointer_state_swap` |
| `mio-capped` | Unsafe calls, `Vec::set_len`, zeroed, transmute, Send/Sync | Platform/layout route quality. | Socket/layout contracts may be platform-specific and human-review-heavy. | `zeroed_*`, `transmute_*`, `unsafe_fn_call_*` |
| `memchr-pr215` | Owner safety-contract inheritance | Tests whether documented unsafe function owners prevent duplicate/noisy operation cards. | Owner inference can drift in nested/long functions. | `long_unsafe_fn_owner_inference`, `documented_private_unsafe_fn` |
| `smallvec-pr407` | Comment-aware owner inference | Tests owner-specific witness routing and comment proximity. | Comments must not become guard evidence. | `local_safety_colon_comment`, `comment_alignment_not_guard` |
| `smallvec-pr277` | `Vec::set_len` shrink/start-bound evidence | Tests shrink evidence and raw-pointer alias review cards. | Shrink cases differ from initialized-extension cases. | `vec_set_len_start_bound_shrink`, `vec_set_len_initialized_loop` |
| `smallvec-pr64` | Raw pointer read and `Vec::set_len` shrink evidence | Tests last-index shrink handling and pointer cards. | Guard evidence must stay tied to the same receiver. | `vec_set_len_last_index_shrink`, `vec_set_len_reassigned_receiver_not_guard` |
| `smallvec-pr254` | Insertion-path pointer arithmetic and raw writes | Tests write-path obligations and capacity/position evidence. | Insert paths often mix capacity, initialized range, and pointer provenance. | `raw_pointer_write_*`, `pointer_arithmetic_*`, `vec_set_len_*` |
| `arrayvec-pr308` | Raw pointer writes in iterator extension | Tests Miri-routed witness prompts for write paths. | A related test mention is not site execution. | `raw_pointer_write_maybeuninit`, `raw_pointer_write_previous_*_not_guard` |
| `arrayvec-pr137` | Raw pointer accessor soundness fix | Tests replacement of reference-derived unchecked access with raw pointer paths. | A soundness fix can add unsafe syntax while improving behavior; review card action, not raw counts. | `get_unchecked_mut_*`, `raw_pointer_*` |
| `arrayvec-pr138` | Safety docs, unsafe fn dedupe, UTF-8 writes | Tests Safety prose as contract evidence and UTF-8/raw write cards. | Safety prose is contract evidence only, not a guard. | `public_unsafe_fn_safety_colon_docs`, `str_from_utf8_unchecked*` |
| `arrayvec-pr187` | Safety prose and len/capacity bounds | Tests raw pointer read bounds evidence. | Length/capacity evidence does not discharge alignment. | `raw_pointer_read_len_capacity_assert`, `align_of_only_not_guard` |
| `arrayvec-pr174` | Inline operation dedupe and `drop_in_place` | Tests operation modeling for drop/deallocation. | Drop ownership and allocation provenance can require human review. | `drop_in_place_deallocation`, `drop_in_place_box_origin` |
| `arrayvec-pr288` | `Vec::set_len`, contract docs, unsafe-call arguments | Tests mixed contract, guard, and unsafe-call evidence. | Unsafe-call preconditions can be callee-specific and should not be generalized. | `vec_set_len_*`, `unsafe_fn_call_*` |
| `hashbrown-pr469` | `unreachable_unchecked` and infallible evidence | Tests infallible-path evidence and owner inference. | Closed or post-call observations must not count as guards. | `unreachable_unchecked_infallible_path`, `unreachable_unchecked_post_infallible_not_guard` |
| `hashbrown-pr501` | Adjacent unchanged unsafe declarations | Tests changed-range discipline. | Zero or fewer cards is useful only if the checkout/diff is aligned. | `adjacent_unchanged_unsafe_fn_no_card` |
| `hashbrown-pr556` | Large changed files | Tests scanner performance without analyzer-truth changes. | Performance dogfood should not be reinterpreted as precision evidence. | syntax scanner tests and capped hashbrown output |
| `hashbrown-pr657` | Multi-line unsafe call wrappers | Tests unsafe-call contract prompts across line breaks. | Wrapper calls need owner context and callee-specific obligations. | `multiline_unsafe_fn_call_wrapper`, `unsafe_fn_call_wrapper` |
| `hashbrown-pr667` | Nested `NonNull::new_unchecked` dedupe | Tests parent-call dedupe and unchecked constructor identity. | Dedupe must not hide distinct unsafe obligations. | `nested_unsafe_operation_call_dedupe`, `nonnull_new_guard` |
| `hashbrown-pr692` | Mutable slice construction and raw writes | Tests `MaybeUninit`, raw write, pointer arithmetic, and private contract evidence. | Slice provenance and initialized range need obligation-specific evidence. | `slice_from_raw_parts_mut*`, `maybeuninit_assume_init*`, `raw_pointer_write_*` |
| `hashbrown-pr681` | Unsafe wrappers and raw pointer dereference | Tests wrapper contract evidence and raw dereference cards. | Replacement branches must preserve exact owner/card identity. | `unsafe_fn_call_wrapper`, `raw_pointer_alignment` |
| `hashbrown-pr693` | `unwrap_unchecked` and infallible result evidence | Tests local infallibility evidence. | Other-result checks and post-checks must not count. | `unwrap_unchecked_infallible_result`, `unwrap_unchecked_other_infallible_not_guard` |
| `bytes-pr826` | `Vec::from_raw_parts` ownership transfer | Tests allocation, capacity, and ownership-transfer wording. | Ownership transfer is often human-review-heavy without receipts. | `vec_from_raw_parts*`, `box_from_raw*` |
| `crossbeam-pr1226` | Strict-provenance cfg and atomic pointer unsafe blocks | Tests cfg-specific contracts and atomic pointer route cards. | cfg/Miri configuration is not proof that all configurations are covered. | `atomic_pointer_state_swap`, `unsafe_impl_send*` |
| `crossbeam-pr1187` | Atomic pointer null swap state transitions | Tests drop/deallocation invariant evidence. | Atomic state transitions can need concurrency witnesses. | `atomic_pointer_state_swap`, `drop_in_place_*` |
| `mio-pr1388` | Socket layout conversions and zeroed/transmute removal | Tests platform layout, raw writes, dereferences, and local Safety comments. | Layout conversions can need sanitizer/proof/human review; comments are not guards. | `zeroed_*`, `transmute_*`, `raw_pointer_write_*`, `local_safety_colon_comment` |

## Updating These Notes

Update this file when a dogfood target changes the reviewer action or records a
new outcome movement. Do not update it just because a fresh local artifact was
generated.

For a rule change, record:

- target id
- operation family or hazard family
- what improved for the reviewer
- what remains noisy or unsupported
- fixture or calibration case that backs the change
- whether support-tier wording changed

If no support-tier wording changed, say so explicitly.
