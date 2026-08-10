# Dogfood report: 2026-08-10 typed repair candidate agent evaluation

Status: issue #1911 PR4 evaluation evidence, pre-release and advisory

This report evaluates the typed `repair_candidates[]` projection merged for
issue #1911 (PRs 1-3) from an agent-consumer point of view. For each observed
candidate it records whether the candidate answers the issue's six questions:
(a) what may change, (b) where, (c) under which preconditions, (d) what is
forbidden, (e) how to verify, and (f) what evidence movement to expect. It also
verifies the two negative rails and converts each observed ambiguity into a
narrow follow-up proposal. No analyzer code was modified; all generated
artifacts remain local and untracked under `target/eval-1911/`.

## Trust boundary

Typed repair candidates are bounded advisory instructions for review or agent
work. They are not patches, not execution results, not witness receipts, not
soundness arguments, and not safety proofs. This report is static
unsafe-contract review evidence over fixture and pinned real inputs. It is
not memory-safety proof, not UB-free status, not Miri-clean status, not
site-execution evidence, not calibrated precision or recall, and not
policy-readiness evidence. No witness ran, no source file was edited by the
tool, and no candidate was applied. This report makes no accuracy claim about
the candidates beyond the recorded field-level observations.

## Inputs and pins

| Input | Value |
|---|---|
| Tool commit (worktree HEAD) | `779ded1715bbc323fd7275e8f393f97d6132138c` (`origin/main`) |
| Tool version | `unsafe-review 0.3.8` |
| Candidate derivation | `crates/unsafe-review-core/src/output/agent/repairs/candidates.rs` |
| Fixture 1 | `fixtures/raw_pointer_alignment` (guard + witness-route candidates) |
| Fixture 2 | `fixtures/public_unsafe_fn_missing_safety` (human-only safety-docs + test candidates) |
| Fixture 3 | `fixtures/raw_pointer_alignment_is_aligned_guard` (auto-applicable test candidate) |
| Pinned-real input | `crossbeam-holdout` checkout at commit `b23b7e8` ("Replace all transmute_copy uses with safer transmute_copy_by_val"), clean worktree |
| Pinned-real fallback | `serde-holdout` at commit `7fc3b4c` (Release 1.0.229); see Deviations |

## Exact commands

```text
cargo build --locked -p unsafe-review-cli

cargo run --locked -p unsafe-review-cli --bin cargo-unsafe-review -- \
  first-pr --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --out-dir target/eval-1911/raw_pointer_alignment

cargo run --locked -p unsafe-review-cli --bin cargo-unsafe-review -- \
  first-pr --root fixtures/public_unsafe_fn_missing_safety \
  --diff fixtures/public_unsafe_fn_missing_safety/change.diff \
  --out-dir target/eval-1911/public_unsafe_fn_missing_safety

cargo run --locked -p unsafe-review-cli --bin cargo-unsafe-review -- \
  first-pr --root fixtures/raw_pointer_alignment_is_aligned_guard \
  --diff fixtures/raw_pointer_alignment_is_aligned_guard/change.diff \
  --out-dir target/eval-1911/raw_pointer_alignment_is_aligned_guard

cargo run --locked -p unsafe-review-cli --bin cargo-unsafe-review -- \
  repo --root <crossbeam-holdout checkout> --format json \
  --out target/eval-1911/crossbeam-holdout-current.json --timeout-seconds 240

cargo run --locked -p unsafe-review-cli --bin cargo-unsafe-review -- \
  context --root <crossbeam-holdout checkout> --json <card-id> \
  > target/eval-1911/crossbeam-context-N.json
```

Candidates were read from `repair-queue.json` / `review-kit.json` /
`lsp.json` (first-pr bundles) and from the `context` agent packet (real
input). Negative rails were additionally checked across every
`fixtures/*/expected.repair-queue.json` (65 candidates).

## Coverage matrix

| Kind \ Applicability | candidate | human_only | requires_witness |
|---|---|---|---|
| `safety_docs` | crossbeam R1, R2 | fixture F2 | — |
| `guard` | fixtures F1, F3; crossbeam R1, R2 | — | — |
| `test` | fixture F3 | fixture F2 | — |
| `witness_route` | — (rail: never) | — (rail: never) | fixtures F1-F3; crossbeam R1-R3 |

All four closed-vocabulary kinds and all three applicability values were
observed on fresh tool output. `human_only` guard candidates were not
observed on fresh inputs (requires a low-confidence or human-review-gated
card in a guard-supporting family); the `human_only` applicability value
itself is covered by fixture F2 safety-docs and test candidates.

## Per-card six-question evaluation

Verdicts: **answered** (machine field present and specific enough to act on),
**ambiguous** (field present but under-specified for an agent), **missing**
(field empty or absent).

### F1 — fixture `raw_pointer_alignment`, guard candidates (`add-raw_pointer_read-{pointer-live,alignment,initialized,allocation}-guard`, applicability `candidate`)

Card site: `src/lib.rs:8:5` (`unsafe { ptr.cast::<Header>().read() }`).

| Question | Verdict | Evidence |
|---|---|---|
| (a) what may change | answered | `allowed_change`: "add a same-origin executable guard for the `alignment` obligation at this card's unsafe site" |
| (b) where | answered | `target.file=src/lib.rs`, `range.start=end={line 8, column 5}`; matches the unsafe block exactly |
| (c) preconditions | answered | obligation description, e.g. "pointer is aligned for the accessed type" |
| (d) forbidden | answered | `SAFETY comment alone`, `debug_assert only`, `broad suppression` |
| (e) verification | answered | `cargo +nightly miri test read_header`, `cargo +nightly careful test read_header` |
| (f) evidence movement | answered | `guard_coverage: missing -> present` |

Note: the range is a zero-width point at the unsafe site, not an edit span
(follow-up 3).

### F1b — same card, witness-route candidate (`attach-witness-receipt`, applicability `requires_witness`)

| Question | Verdict | Evidence |
|---|---|---|
| (a) what may change | answered | "attach a scoped witness receipt after running the suggested command outside unsafe-review" |
| (b) where | answered | same card site target (`src/lib.rs:8:5`) |
| (c) preconditions | ambiguous | "the selected witness route remains unconfirmed" — the route kind and suggested command are not in the candidate (follow-up 2) |
| (d) forbidden | answered | "treating a suggested command as an executed witness", "using an unrelated receipt as proof" |
| (e) verification | answered | card verify commands (miri/careful) present |
| (f) evidence movement | answered | `witness_receipt_coverage: missing -> present` |

### F2 — fixture `public_unsafe_fn_missing_safety`, safety-docs and test candidates (`add-safety-contract`, `add-focused-test`, both applicability `human_only`)

Card site: `src/lib.rs:1:1` (`pub unsafe fn caller_must_uphold_contract()`).

| Question | Verdict | Evidence |
|---|---|---|
| (a) what may change | answered | "add or expose the local safety contract for this card's obligations" / "add or point to a focused test that exercises this owner or seam" |
| (b) where | answered | `src/lib.rs:1:1`; matches the public unsafe declaration exactly |
| (c) preconditions | answered | "the contract obligation remains undischarged" / "the owner or unsafe seam lacks focused test reach evidence" (generic but machine-readable) |
| (d) forbidden | answered | includes "claiming a contract is proof" (docs) and "test mention without exercising the unsafe owner" (test) |
| (e) verification | missing | `verification: []` on both candidates (follow-up 1) |
| (f) evidence movement | answered | `contract_coverage` / `test_reach_coverage: missing -> present` |

### F3 — fixture `raw_pointer_alignment_is_aligned_guard`, test candidate (`add-focused-test`, applicability `candidate`)

Card site: `src/lib.rs:11:5` (unsafe block after the present alignment guard).

| Question | Verdict | Evidence |
|---|---|---|
| (a) what may change | answered | as F2 test candidate |
| (b) where | answered | `src/lib.rs:11:5`; matches the unsafe block exactly |
| (c) preconditions | answered | as F2 |
| (d) forbidden | answered | as F2 |
| (e) verification | answered | `cargo +nightly miri test read_header`, `cargo +nightly careful test read_header` |
| (f) evidence movement | answered | `test_reach_coverage: missing -> present` |

### R1 — pinned-real crossbeam, `crossbeam-channel/src/flavors/array.rs:221` (`raw_pointer_deref`, class `contract_missing`)

Candidates: `add-safety-contract` (candidate), four `add-raw_pointer_deref-*-guard`
(candidate), `attach-witness-receipt` (requires_witness).

| Question | Verdict | Evidence |
|---|---|---|
| (a) what may change | answered | per-candidate `allowed_change` strings as in fixtures |
| (b) where | answered | `target.range.start=end={line 221, column 40}`; line 221 is `unsafe { slot.msg.get().write(MaybeUninit::new(msg)) }`; column points into the unsafe expression |
| (c) preconditions | answered | obligation-specific text, e.g. "buffer has enough bytes for the accessed type", "access remains inside one live allocation" |
| (d) forbidden | answered | closed substitute lists per kind |
| (e) verification | answered | `cargo +nightly miri test write`, `cargo +nightly careful test write` |
| (f) evidence movement | answered | `contract_coverage` / `guard_coverage` / `witness_receipt_coverage: missing -> present` |

### R2 — pinned-real crossbeam, `array.rs:315` (`maybe_uninit_assume_init`, class `contract_missing`)

Candidates: `add-safety-contract`, `add-maybe_uninit_assume_init-initialized-guard`,
`attach-witness-receipt`. All six questions answered with the same shape as
R1; guard precondition is family-specific ("all fields/elements are
initialized and valid before `assume_init`"); verification
`cargo +nightly miri test read` / `careful test read`.

### R3 — pinned-real crossbeam, `crossbeam-epoch/src/atomic.rs:780` (public `unsafe fn into_owned`, class `guarded_unwitnessed`)

Only candidate: `attach-witness-receipt` (requires_witness). No safety-docs
candidate is emitted because the contract obligation is already discharged —
consistent with the conservative derivation (no fabricated candidates).

| Question | Verdict | Evidence |
|---|---|---|
| (a) what may change | answered | attach receipt after external run |
| (b) where | answered | `atomic.rs:780:5`; matches `pub unsafe fn into_owned` exactly |
| (c) preconditions | ambiguous | "the selected witness route remains unconfirmed" — route not named (follow-up 2) |
| (d) forbidden | answered | as F1b |
| (e) verification | missing | `verification: []` (follow-up 1) |
| (f) evidence movement | answered | `witness_receipt_coverage: missing -> present` |

### Tally

Across the 7 evaluated candidate groups (42 question cells):

| Verdict | Count |
|---|---:|
| answered | 37 |
| ambiguous | 2 (both: witness-route precondition does not name the route) |
| missing | 3 (all: empty `verification` on F2 safety-docs, F2 test, R3 witness) |

## Negative rails

Rail 1 — human-only or witness-required work never receives an auto-applicable
candidate. Across 65 candidates in all `fixtures/*/expected.repair-queue.json`,
3 fresh first-pr bundles, and 3 real context packets:

- every `witness_route` candidate has applicability `requires_witness` (0 violations);
- no candidate in a `requires_human_review` or `do_not_auto_repair` bucket has applicability `candidate` (0 violations);
- all candidates on the human-only public-unsafe-declaration card (F2) are `human_only` or `requires_witness`;
- kind and applicability vocabularies are closed: only `{safety_docs, guard, test, witness_route}` x `{candidate, human_only, requires_witness}` observed.

Nuance recorded: repair-queue buckets are card-level while applicability is
candidate-level. A card in the `requires_witness_receipt` bucket still carries
guard candidates with applicability `candidate` when non-witness work remains
(readiness gate 3 in `coverage.rs` only receipt-gates cards whose *entire*
missing set is witness). This is internally consistent but easy to mis-skim
(follow-up 5).

Rail 2 — no candidate is presented as already applied, correct, sufficient, or
safety-proving. Every observed candidate carries exactly the constant
`claim_boundary`: "advisory repair candidate only; not a patch, execution
result, witness receipt, proof, or safety claim". A scan of all generated
bundles for applied/correct/sufficient/proof language found no violation.

## Consumer parity

For each fixture bundle, the `repair_candidates` blocks in
`review-kit.json` (`handoff.review_cards.card_queue`) and `lsp.json` are
byte-equal (after key ordering) to the `repair-queue.json` blocks — no
reclassification across consumers. The `repo --format json` card array does
not carry `repair_candidates` at all (follow-up 4).

## Follow-up proposals

1. Seam: `push_candidate` copies `card.next_action.verify_commands` unchecked
   (`candidates.rs:199`). Observed: `verification: []` on F2 (`public_unsafe_fn_missing_safety`)
   and R3 (`guarded_unwitnessed`) candidates. Expected: a verify command or an
   explicit machine reason none exists; actual: silently empty array.
2. Seam: witness-route `CandidateSpec` (`candidates.rs:154-170`). Observed:
   preconditions say "the selected witness route remains unconfirmed" and
   `allowed_change` says "the suggested command" on F1b/R3. Expected: route
   kind and suggested command copied from `card.routes`; actual: the agent
   must join the candidate to card fields to learn which route.
3. Seam: `target()` (`candidates.rs:210-222`). Observed:
   `range.start == range.end` on every candidate. Expected: documented
   point-anchor semantics (site anchor; edit placement is agent judgment) or
   an explicit insertion hint; actual: a zero-width range whose intent is
   unstated.
4. Seam: repo-scan JSON projection. Observed: `repo --format json` cards on
   serde (6 cards) and crossbeam (693 cards) carry no `repair_candidates`;
   candidates exist only in repair-queue, review-kit, LSP, and context
   packets. Expected per the #1880 consumer map: projection or a documented
   exclusion; actual: absent without note.
5. Seam: repair-queue bucket naming vs candidate applicability. Observed:
   guard candidates with applicability `candidate` sit inside the
   `requires_witness_receipt` bucket (F1, F3). Expected: a doc note that
   candidate-level `applicability` is authoritative over bucket placement;
   actual: readers must infer the two-level rule.

## Artifact receipt

Generated artifacts are local-only under `target/eval-1911/` (untracked).
Hashes preserve reproducible identity without committing output.

| Artifact | SHA-256 |
|---|---|
| `raw_pointer_alignment/repair-queue.json` | `5ca92dcf9b5af42d8d7a8cc199fdd63c6bbb607c2aa6d5a13b742309f611673d` |
| `public_unsafe_fn_missing_safety/repair-queue.json` | `15b1b88af0d5e589178ce5ea900d529742c3e5f500771fc690ffc963b2b596d5` |
| `raw_pointer_alignment_is_aligned_guard/repair-queue.json` | `3ec8e5193e6141669389eb9b6db31d341ba91af4f18cdd6c33eaa433af77ed49` |
| `raw_pointer_alignment/review-kit.json` | `110433a578053d906d1dd72f06265b8231bd3f0fa007a2e300265b636eb021bb` |
| `crossbeam-context-1.json` | `4379a5593424ded11b506af7b5fbed18806860aa2daae556feb282a90e8b8ff2` |
| `crossbeam-context-2.json` | `7df3e84454418501a1952b2fe18ddc57c9aa1c2c5e92b39e5e214cdc33209129` |
| `crossbeam-context-3.json` | `e36661b0ccb89be91eabeb80eda992b18ddaf3c7c715cebe1a2b761f9333503b` |
| `crossbeam-holdout-current.json` | `d6f21215fa182be219cd6b60f1d38b9751ba09241c99b539d3a418bcdf3f226f` |
| `serde-holdout-current.json` | `fb4c8f769cf2bbac18a2baef34cb463a5b633e7bd6cb8a1165154dfbfbad04e7` |
| `serde-holdout.unsafe-review.json` (copied pin) | `73ab56566faa4477eeeb0bfdb1d6fd3e4f99e829074d64aa8405591ae27cab4f` |

## Deviations

- The planned `serde-holdout` pinned checkout
  (`unsafe-review-swarm-1891/target/dogfood-work/serde-holdout`) was deleted
  by the concurrent #1891 dogfood agent after this evaluation completed a
  fresh full-repo scan of it (`serde-holdout-current.json`, 6 cards) but
  before per-card `context` packets could be captured. The pre-existing
  pinned artifact (`serde-holdout.unsafe-review.json`, tool 0.3.8,
  generated 2026-08-10T19:19:48Z) was copied first and contains no
  `repair_candidates` because repo-scan JSON does not project them
  (follow-up 4). The pinned crossbeam-holdout checkout under the primary
  checkout's `target/dogfood-work/` was substituted read-only as the
  pinned-real input; no file in that checkout was modified.
- `human_only` guard candidates were not reachable on the selected fresh
  inputs; the `human_only` value is covered by F2 safety-docs/test candidates
  and by the derivation's unit tests.
