# #2224 PR1 batch report: first real-PR seam counts

Status: agent-drafted source-first batch. Draft evidence, not human
adjudication, not calibration. All counts below are recomputed by
`cargo run --locked -p xtask -- check-seam-batch` from `seams.toml`,
`mapping.toml`, and the frozen outputs; the check fails if this report's
quoted totals disagree with its rows (via the mapping tally).

Analyzer: unsafe-review 0.3.8 at development revision
`8ff3bd9ae6107076d1289258f44c2be130c73003` (origin/main at batch time).
The published-0.4.0 fixed-comparison slot is reserved and unfilled because
0.4.0 is not yet published.

## Raw counts (frozen inventory: 8 expected seams, 13 emitted cards)

| Measure | Numerator | Denominator | Value |
|---|---|---|---|
| Seam recall (match / expected) | 8 | 8 | 8/8 |
| Operation-family accuracy | 7 | 8 | 7/8 |
| Obligation correctness (no over-credit, no missed guard/evidence) | 4 | 8 | 4/8 |
| Obligation over-credit | 0 | 8 | 0/8 |
| Reviewer-useful cards | 7 | 13 | 7/13 |
| Duplicates | 2 | 13 | 2/13 |
| Wrongly-surfaced cards | 3 | 13 | 3/13 |
| Quiet-control cards (arrayvec-143) | 0 | — | 0 |

No rates are claimed. With 8 seams in 5 cases these are existence proofs
and failure-shape notes, not precision or recall.

## Per-case slice

| Case | Expected | Matched | Missing | Emitted | Useful |
|---|---|---|---|---|---|
| arrayvec-143 (quiet control) | 0 | — | — | 0 | — |
| hashbrown-692 | 1 | 1 | 0 | 3 | 2 |
| memchr-226 | 1 | 1 | 0 | 1 | 1 |
| getrandom-811 | 1 | 1 | 0 | 1 | 1 |
| arrayvec-138 | 5 | 5 | 0 | 8 | 3 |

## Per-family slice

| Family | Seams | Found | Obligation-correct |
|---|---|---|---|
| slice_from_raw_parts_mut | 1 | 1 | 1 |
| pointer_arithmetic | 2 | 2 | 1 |
| ffi | 1 | 1 | 0 |
| raw_pointer_write | 2 | 2 | 1 |
| unsafe_fn_call | 2 | 2 | 0 |

## What the batch establishes

- The tool finds every inventoried seam, including the quiet-control
  zero: no silent misses in this batch, no false alarms on safe-only code.
- Guarded writes stay silent correctly (encode_utf8 body arms), and the
  MaybeUninit return type is honored in effect (init dropped from the
  next action).
- Missing-contract detection fires exactly where the contract is missing
  (new `write` helper, try_push call site).

## What the batch refutes or limits

- Obligation understanding is the weak axis (4/8): two missed guard credits
  (memchr distance check, getrandom return-value check), two uncredited
  test-harness discharge setups. Zero over-credit coexists with missed
  credit; strictness is not understanding.
- The getrandom card never names return-value validation, the obligation
  the PR fixes. A reviewer relying on obligation lists would miss the point
  of the change.
- Card granularity noise is real: three cards on one helper (S1), a
  declaration card where the seam is a write cluster (S2).
- Usefulness is 7/13: the test-harness calls (S4/S5), the declaration
  restatement (S2), and the pre-existing context arithmetic are found but
  not worth surfacing.

## Inventory misses (challenge-adjusted tally)

The frozen inventory missed two genuine seams the tool found: the
`fill_tag` write_bytes site (diff context hid the operation line) and the
`set_len` unsafe call (mislabeled safe). Both are recorded as
inventory-challenge cards, keeping the frozen tally intact. Challenge
adjusted: 10/10 found. The diff-scoping lesson for PR2: inventory needs
the full enclosing function, not just the hunk, before the analyzer runs.

## Unresolved judgments and their effect

- memchr anchor precision: match chosen; a line-exact reading gives
  missing + wrongly-surfaced instead (recall 7/8, false alarms 4/13).
- A138-S3 struct-invariant discharge: stays missing; crediting the
  unenforced invariant would move obligations-correct to 5/8 without
  evidence.
- Label vocabulary used non-canonical family names (ffi_call, unsafe_call);
  canonical tool families are ffi and unsafe_fn_call. No count effect; PR2
  needs a canonical family list for labelers.

## Uncertainty and limits

- n=8 seams across 5 cases; arrayvec-138 contributes 5/8 (case dominance).
- Samples are correlated by project and by labeler (single agent drafter).
- Unsupported families, parse failures, and partial runs: zero in this
  batch; the evaluator supports `unknown` outcomes and the tests probe them.
- Empty denominators stay unknown: the quiet control has no recall by rule.
