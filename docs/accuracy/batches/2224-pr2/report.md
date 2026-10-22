# #2224 PR2 batch report: growth-batch seam counts
#
# Status: agent-drafted source-first batch. Draft evidence, not human
# adjudication, not calibration. All counts below are recomputed by
# `cargo run --locked -p xtask -- check-seam-batch` from `seams.toml`,
# `mapping.toml`, and the frozen outputs; the quoted totals match the
# mapping tally exactly.
#
# Analyzer: unsafe-review 0.3.8 at development revision
# `09d30034fca6983a860bb94126b657d4f962aec9` (origin/main at batch time).
# The published-0.4.0 fixed-comparison slot is reserved and unfilled because
# 0.4.0 is not yet published.
#
# ## Raw counts (frozen inventory: 9 expected seams, 30 emitted cards)
#
# | Measure | Numerator | Denominator | Value |
# |---|---|---|---|
# | Seam recall (match / expected) | 9 | 9 | 9/9 |
# | Operation-family accuracy | 8 | 9 | 8/9 |
# | Obligation correctness (no over-credit, no missed guard/evidence) | 2 | 9 | 2/9 |
# | Obligation over-credit | 0 | 9 | 0/9 |
# | Reviewer-useful cards | 10 | 30 | 10/30 |
# | Duplicates | 15 | 30 | 15/30 |
# | Wrongly-surfaced cards | 6 | 30 | 6/30 |
# | Quiet-control cards (bytes-744) | 0 | — | 0 |
#
# No rates are claimed. With 9 seams in 6 cases these are existence proofs
# and failure-shape notes, not precision or recall.
#
# ## Per-case slice
#
# | Case | Expected | Matched | Missing | Emitted | Useful |
# |---|---|---|---|---|---|
# | crossbeam-1287 (Send/Sync bounds) | 1 | 1 | 0 | 2 | 1 |
# | crossbeam-1297 (epoch SB fix) | 2 | 2 | 0 | 14 | 2 |
# | bytes-597 (MaybeUninit BufMut) | 3 | 3 | 0 | 6 | 3 |
# | bytes-818 (extend_from_within) | 2 | 2 | 0 | 4 | 3 |
# | bytes-744 (quiet control) | 0 | — | — | 0 | — |
# | parking_lot-349 (guard Send/Sync) | 1 | 1 | 0 | 4 | 1 |
#
# ## Per-family slice (canonical tool families)
#
# | Family | Seams | Found | Obligation-correct |
# |---|---|---|---|
# | unsafe_impl_send_sync | 2 | 2 | 0 |
# | raw_pointer_deref | 1 | 1 | 1 |
# | unsafe_fn_call | 3 | 3 | 0 |
# | unsafe_declaration | 1 | 1 | 1 |
# | raw_pointer_write | 1 | 1 | 0 |
# | copy_nonoverlapping | 1 | 1 | 0 |
#
# The unsafe_declaration row needs a footnote: the tool family on the
# bytes-597-S1 primary is `unknown`, so family accuracy counts it wrong
# (8/9) while obligation correctness counts it right. The tool has no
# operation family for unsafe trait impls outside Send/Sync.
#
# ## What the batch establishes
#
# - The tool finds every inventoried seam across three new projects, six
#   cases, and six operation families, including two cfg-gated unsafe
#   impls (parking_lot-349) it does not skip and a second quiet control
#   with zero cards. No silent misses in this batch.
# - Missing-contract detection fires exactly where contracts are missing:
#   the new BufMut impl, both Send/Sync soundness fixes, and the raw
#   unpin cluster all surface with contract-missing evidence.
# - Zero over-credit coexists with deep missed credit again: strictness is
#   not understanding, now at 2/9 after PR1's 4/8.
#
# ## What the batch refutes or limits
#
# - Guard credit is the weak axis and it got weaker on new ground. The
#   tool misses same-screen establishment systematically: SAFETY comments
#   plus asserts (bytes-818-S1), a dominating assert (bytes-597-S3), a
#   panicking bounds check (bytes-597-S2), a dominating null check
#   (crossbeam-1297-S2), and signature where-clauses (both Send/Sync
#   fixes). The bytes-818-S1 next action even demands guards that exist.
# - Card granularity noise dominates the emission population: 15 of 30
#   cards are duplicates. One five-line epoch function draws ten cards
#   (declaration, call-shape, and deref views of the same four lines);
#   each unsafe impl draws a wrong-family declaration twin on its #[cfg]
#   attribute line. Usefulness is 10/30 on that arithmetic alone.
# - Context-line cards recur: three cards sit on pre-existing lines the
#   sampled PRs never touched (bytes extend_from_slice, epoch repin
#   path, &[u8] put_bytes). Diff scoping keeps them out of no seam, but
#   a reviewer still reads them.
# - verify_commands hallucinate test names: ArcMutexGuard_loom/shuttle,
#   miri/careful put_bytes, miri/careful unpin name tests that do not
#   exist in the sampled repos. The documented actions stay feasible;
#   the verification strings do not verify.
#
# ## Inventory misses (challenge-adjusted tally)
#
# The frozen inventory missed two genuine seams the tool found: the
# `as_ptr().add(begin)` arithmetic feeding the bytes-818 copy (folded
# into S1's anchor only implicitly) and the test-helper transmute behind
# the bytes-597 MaybeUninit tests (mislabeled safe). Both are recorded as
# inventory-challenge cards, keeping the frozen tally intact. Challenge
# adjusted: 10/11 found (the add site is reviewer-useful; the sound
# commented test transmute is not).
#
# One further scope note, not a challenge row: bytes-597 also adds an
# `unsafe impl BufMut for Special` test scaffold with unreachable!()
# bodies. Neither the inventory nor the tool covers it; it is test-only
# scaffolding with no live seam, recorded here as an excluded
# consideration rather than a silent omission.
#
# ## Unresolved judgments and their effect
#
# - 1297-S1 cross-function discharge: correct chosen. The pinning
#   establishment lives at the guard.rs call sites, which a
#   definition-site card cannot see; penalizing that would demand
#   inter-procedural analysis. The strict reading gives
#   obligations-correct 1/9.
# - 597-S1 impl granularity: correct chosen. Discharge is genuinely
#   method-local, so the impl-header card reporting missing discharge is
#   accurate at its granularity; judging the whole impl by the
#   per-method misses would give 1/9.
# - Unknown-family duplicates: duplicate chosen. The two parking_lot
#   attribute-line cards name the wrong family on matched anchors. A
#   wrongly-surfaced reading is defensible and would move the split to
#   13 duplicates / 8 wrongly-surfaced with no count-effect on recall.
# - No canonical obligation key covers Send/Sync bound adequacy; the
#   nearest memory keys (pointer-live, allocation) are documented
#   approximations in the seam rows, and that vocabulary gap is itself a
#   PR2 finding for the obligation model.
#
# ## Uncertainty and limits
#
# - n=9 seams across 6 cases; crossbeam-1297 contributes 14/30 cards
#   (emission dominance, not seam dominance).
# - Samples are correlated by project (two cases each from crossbeam and
#   bytes) and by labeler (single agent drafter).
# - Unsupported families, parse failures, and partial runs: zero in this
#   batch; the evaluator supports `unknown` outcomes and the PR1 tests
#   probe them.
# - Empty denominators stay unknown: the quiet control has no recall by
#   rule.
