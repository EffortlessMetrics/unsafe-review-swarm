# #2224 PR3 batch protocol: matching rules and baseline questions
#
# Status: batch-scoped. These rules govern only
# `docs/accuracy/batches/2224-pr3/`. They do not define global precision or
# recall. Counting rules are inherited from the PR1 protocol
# (`docs/accuracy/batches/2224-pr1/protocol.md`): one disposition per emitted
# card, one outcome per expected seam, the evaluator recomputes every count,
# quiet-control cards are wrongly-surfaced by rule, and empty denominators
# stay unknown. Frozen first-run outputs (unsafe-review 0.3.8 at lane
# revision `30c4b9eb`, one run per case with the manifest command shape)
# were acquired after the source-first inventory was committed; `mapping.toml`
# carries match/missing judgments against those bytes.
#
# Vocabulary is locked, not per-batch: labelers use ONLY the sets in
# `docs/accuracy/OPERATION-FAMILY-OBLIGATION-SCHEMA.toml` (schema v1):
# canonical `OperationFamily::as_str` families, canonical obligation keys from
# `obligations_for`, and hazard slugs on the card-identity axis only. PR1
# legacy names are frozen history (see the schema's legacy table). Where no
# canonical member covers the review question, the seam rationale says so
# explicitly (schema known-gaps) instead of stretching a member to fit.
#
# PR3 additions over PR2:
#
# - Zero-seam roles beyond quiet. bytemuck-349 (hardening docs + regression
#   test) and smallvec-757 (test scaffolding + generated-input fixture) carry
#   zero expected seams like the quiet control: their changes add no new or
#   changed unsafe operation. Cards on their safe-API test calls are
#   wrongly-surfaced, not recall misses. The roles stay distinct in the
#   report (contract-hardening vs fixture-scaffolding vs quiet) even though
#   counting treats all three as zero-seam cases.
# - Debug-only guard note. smallvec-768-S1 judges whether the card respects
#   the debug-only limit of `debug_assert!(length <= INLINE)` (PR1 getrandom
#   lesson carried forward).
# - Partial-run note. smallvec-757's frozen diff adds `tests/proptest.rs`,
#   but the pinned head tree no longer contains that file (the head commit
#   removes it), so the analyzer scanned only the resolved files and warns
#   accordingly. The unscanned file is named in the report; it carries no
#   seam either way (safe-only test scaffolding).
# ## The four baseline questions
#
# Every expected seam answers all four in `mapping.toml`:
#
# 1. **Found + operation (Q1).** Did the analyzer emit a card covering the
#    seam's source anchor with the correct canonical operation family?
# 2. **Obligations without over-credit (Q2).** Did the card name the seam's
#    expected obligations, and did it avoid crediting evidence the source does
#    not provide (wrong-shape guards, comments as discharge, removed asserts,
#    debug-only guards as release guarantees)?
# 3. **Useful selection (Q3).** Would a reviewer working this PR scope want
#    this card surfaced? Safe-API regression tests are genuine scope but
#    usually not useful selections; the mapping says so explicitly instead of
#    deleting them from the denominator.
# 4. **Next action (Q4).** Was the proposed next action correct, sufficiently
#    specific to this seam, and feasible (no witness execution assumed, no
#    third-party writes, no commands that cannot run in this scope)?
#
# ## Ordering rule
#
# `seams.toml` is recorded from source reading before analyzer output is
# consulted (`analyzer_outputs_consulted = false`). Mapping never edits the
# seam inventory to fit the output; disagreements become `missing` /
# `wrongly-surfaced` rows or visible unresolved judgments with their effect
# on counts stated.
