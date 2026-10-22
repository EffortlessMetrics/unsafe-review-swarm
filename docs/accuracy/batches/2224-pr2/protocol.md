# #2224 PR2 batch protocol: matching rules and baseline questions
#
# Status: batch-scoped. These rules govern only `docs/accuracy/batches/2224-pr2/`.
# They do not define global precision or recall. Counting rules are inherited
# from the PR1 protocol (`docs/accuracy/batches/2224-pr1/protocol.md`):
# one disposition per emitted card, one outcome per expected seam, the
# evaluator recomputes every count, quiet-control cards are wrongly-surfaced
# by rule, and empty denominators stay unknown.
#
# PR2 additions and corrections over PR1:
#
# - Canonical operation families. Labelers use the snake_case names from
#   `OperationFamily::as_str` in
#   `crates/unsafe-review-core/src/domain/operation.rs` (raw_pointer_deref,
#   raw_pointer_write, pointer_arithmetic, copy_nonoverlapping,
#   unsafe_fn_call, unsafe_impl_send_sync, ffi, unsafe_declaration,
#   maybe_uninit_assume_init, vec_set_len, and the rest of that table).
#   PR1 used non-canonical names (ffi_call, unsafe_call); PR2 does not.
# - Canonical obligation keys. Labelers use the hyphenated keys constructed
#   in `crates/unsafe-review-core/src/analysis/obligations/safety.rs`:
#   initialized, valid-range, pointer-live, capacity, allocation, alignment,
#   utf8, non-overlap, layout, bounds. PR1 used underscore names; PR2 does
#   not. Where no canonical key covers the review question (Send/Sync bound
#   adequacy for unsafe impls), the seam rationale says so explicitly
#   instead of stretching a memory key to fit.
# - Full enclosing function before inventory. PR1 learned that hunk-only
#   reading hides operations in context lines (fill_tag write_bytes, set_len
#   call). Every PR2 seam anchor was read with its enclosing function at the
#   pinned head before any analyzer output was consulted.
# - Contract versus guard. `expected_contract` records whether the source
#   carries a safety contract for the seam (SAFETY comment, documented
#   unsafe-fn obligations, or structural bounds in the signature).
#   `expected_discharge` records whether the code establishes the
#   obligation (dominating assert, panicking bounds check, dominating
#   null check). An assert is a guard, not a contract; a SAFETY comment is
#   a contract, not a guard. Q2 judges whether the card respects that
#   split in both directions: no credit for removed or non-dominating
#   checks, no silence demanded where the obligation is genuinely open.

## The four baseline questions

Every expected seam answers all four in `mapping.toml`:

1. **Found + operation (Q1).** Did the analyzer emit a card covering the seam's
   source anchor with the correct canonical operation family?
2. **Obligations without over-credit (Q2).** Did the card name the seam's
   expected obligations, and did it avoid crediting evidence the source does
   not provide (wrong-shape guards, comments as discharge, removed asserts)?
3. **Useful selection (Q3).** Would a reviewer working this PR scope want this
   card surfaced? Safe-API regression tests are genuine scope but usually
   not useful selections; the mapping says so explicitly instead of deleting
   them from the denominator.
4. **Next action (Q4).** Was the proposed next action correct, sufficiently
   specific to this seam, and feasible (no witness execution assumed, no
   third-party writes, no commands that cannot run in this scope)?

## Ordering rule

`seams.toml` is recorded from source reading before analyzer output is consulted
(`analyzer_outputs_consulted = false`). Mapping never edits the seam inventory
to fit the output; disagreements become `missing` / `wrongly-surfaced` rows or
visible unresolved judgments with their effect on counts stated.
