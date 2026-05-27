# Dogfood report: 2026-05-26 post-burst analyzer snapshot

Status: experimental snapshot report
Swarm commit: `5aca416`
Artifact status: local, untracked under `target/dogfood-work/`

This report samples the post-burst analyzer behavior after
[the analyzer audit](../../handoffs/2026-05-26-post-burst-analyzer-audit.md).
Notable observations are classified with the
[dogfood triage taxonomy](../triage-taxonomy.md) so follow-up work starts from
reviewer usefulness instead of raw card counts.
It is not a support-tier promotion, calibration report, release readiness
proof, policy decision, safety proof, UB-free claim, Miri-clean claim, or site
execution proof.

## Scope

Selected targets:

- `arrayvec-pr137`
- `arrayvec-pr288`
- `hashbrown-pr693`
- `memchr-capped`
- `crossbeam-pr1226`
- `mio-pr1388`

Command profile:

- PR targets used `unsafe-review check --format json`.
- `memchr-capped` used `unsafe-review repo --format json`.
- Max cards were the manifest caps for each selected target.
- Raw PR patches and JSON outputs were generated under `target/dogfood-work/`.
- No witness tools were run.
- No receipts were imported.

Root commits:

| Target | Root commit |
|---|---|
| `arrayvec-pr137` | `fd72321` |
| `arrayvec-pr288` | `cd11fb5` |
| `hashbrown-pr693` | `ea6e08b` |
| `memchr-capped` | `db1a77d` |
| `crossbeam-pr1226` | `20b25e3` |
| `mio-pr1388` | `8d3ed77` |

## Summary

| Target | Cards | Families | Obvious useful cards | Obvious noise | Notes |
|---|---:|---|---|---|---|
| `arrayvec-pr137` | 16 | `raw_pointer_read`, `vec_set_len`, `pointer_arithmetic`, `raw_pointer_write`, `drop_in_place`, `ptr_copy`, `slice_from_raw_parts`, `unknown` | Soundness-fix PR produces concrete raw-pointer and `Vec::set_len` review cards instead of a raw unsafe count. | One `unknown` unsafe-fn card and broad `contract_missing` cards can be noisy without reviewer context. | Good target for "same receiver / same pointer" applicability and for reminding docs that unsafe count can rise during a fix. |
| `arrayvec-pr288` | 8 | `vec_set_len`, `unsafe_fn_call` | `Vec::set_len` cards mostly moved to `guarded_unwitnessed`, which gives the reviewer a witness/action route instead of a generic missing-guard complaint. | One `try_push_str` `vec_set_len` card remained `guard_missing` in this snapshot; a focused [Vec::set_len rerun](2026-05-26-arrayvec-vec-set-len-rerun.md) now moves it to `guarded_unwitnessed`. | Keep it as an initialized-range regression target; add new fixtures only for future stale or wrong-target shapes. |
| `hashbrown-pr693` | 15 | `unwrap_unchecked`, `unsafe_fn_call`, `nonnull_unchecked`, `raw_pointer_read` | `unwrap_unchecked` cards include `guarded_unwitnessed` local infallibility evidence, which is exactly the intended reviewer note shape. | Nearby unsafe-call and `NonNull` cards remain mixed with the unwrap cards; ranking/summary should keep the unwrap evidence easy to find. | Good target for same-receiver and open-branch checks. |
| `memchr-capped` | 50 | `unknown`, `unsafe_fn_call`, `target_feature`, `pointer_arithmetic` | `target_feature` cards remain `guarded_unwitnessed`, preserving the "contract exists, witness still absent" posture. | 24 `unknown` owner/unsafe-fn cards make the capped snapshot inventory-like. | Useful as a capped target-feature regression check, not as precision evidence. |
| `crossbeam-pr1226` | 6 | `unknown` | The original snapshot pointed at changed atomic unsafe blocks in `fetch_and`, `fetch_or`, and `fetch_xor`. | The snapshot cards were generic `unknown` `contract_missing`; a focused [atomic pointer rerun](2026-05-26-crossbeam-atomic-pointer-rerun.md) now classifies those operations as `atomic_pointer_state`. | Keep it as an atomic pointer/state regression target; do not turn the classifier into a concurrency proof. |
| `mio-pr1388` | 18 | `unsafe_fn_call`, `zeroed`, `raw_pointer_deref`, `raw_pointer_write`, `ffi` | Layout/zeroed/raw-pointer cards point to concrete socket-address conversion review work. | The mix of `contract_missing`, `guard_missing`, and one `miri_unsupported` FFI card needs reviewer wording to avoid implying Miri coverage. | Good platform/layout target for human-review-heavy route wording. |

## Triage observations

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `arrayvec-pr288` | `vec_set_len` `try_push_str` card | `actionable` | The focused rerun moves the remaining `guard_missing` to `guarded_unwitnessed`, preserving a witness/action route instead of a missing-guard prompt. | Keep the existing applicability refactors as regression pressure; add a fixture only for a future stale or wrong-target initialized-range shape. |
| `crossbeam-pr1226` | changed atomic unsafe blocks | `actionable` | The original snapshot had six generic `unknown` cards for atomic pointer/state fetch operations; the focused rerun now reports six `atomic_pointer_state` `requires_loom` cards. | Keep the existing fixtures as regression coverage; add another fixture only if a future rerun exposes a still-missing concrete atomic pointer/state shape. |
| `memchr-capped` | `unknown` unsafe-fn owner cards | `noise` | Twenty-four `unknown` cards make the capped run inventory-like rather than PR-review focused. | The focused [comment-plan follow-up](2026-05-26-memchr-unknown-comment-plan.md) keeps unknown-family cards out of inline comment candidates while preserving them in the advisory bundle. |
| `memchr-capped` | `target_feature` | `actionable` | Target-feature cards preserve contract evidence while leaving witness evidence absent. | Keep `target_feature_safety_docs` as the guardrail; do not turn docs into availability proof. |
| `mio-pr1388` | FFI/platform layout cluster | `needs-route` | The FFI card is correctly `miri_unsupported`, but surrounding layout cards need human-review-heavy wording. | The focused [FFI route wording follow-up](2026-05-26-mio-ffi-route-wording.md) adds a fixture verifier for FFI boundary contract wording and Miri limitation text. |
| selected corpus | safe/no-unsafe control | `needs-doc` | The sampled real-crate corpus has no explicit zero-card/no-unsafe target; the separate [`no-card fixture smoke`](2026-05-26-no-card-control.md) records the fixture-level false-positive control. | Keep the fixture control linked from the dogfood README; do not treat it as real-crate precision evidence. |
| `arrayvec-pr137` | soundness-fix card count | `needs-doc` | The PR can look worse by card count even when it is a soundness-oriented upstream change. | Keep docs/reports centered on actionability rather than raw gap deltas. |

## Findings by family

### `Vec::set_len`

`arrayvec-pr288` is the most useful target. It produced seven `vec_set_len`
cards: most were `guarded_unwitnessed`, with one remaining `guard_missing`
around `try_push_str`. The focused
[Vec::set_len rerun](2026-05-26-arrayvec-vec-set-len-rerun.md) now moves that
card to `guarded_unwitnessed`, so the next fixture should come from a future
stale or wrong-target initialized-range shape rather than this already-covered
dogfood shape.

`arrayvec-pr137` produced four `vec_set_len` `contract_missing` cards in a PR
that intentionally changes unsafe internals. Treat those as reviewer prompts,
not a regression count.

### `unwrap_unchecked`

`hashbrown-pr693` produced eight `unwrap_unchecked` cards, including
`guarded_unwitnessed` cards where local infallibility evidence was detected.
This supports the current audit direction: before adding more unwrap patterns,
factor same-receiver and open-branch applicability so these cards stay precise.

### `target_feature`

`memchr-capped` still shows ten `target_feature` cards as `guarded_unwitnessed`.
That is the right posture: contract evidence can improve the reviewer note, but
it is not target-feature availability proof, site-execution proof, or Miri
evidence.

### Generic `unknown`

`memchr-capped` and the original `crossbeam-pr1226` snapshot show the main
post-burst noise pocket. `memchr-capped` has broad owner/unsafe-fn cards that
are useful as inventory, while `crossbeam-pr1226` exposed changed atomic unsafe
blocks without a specific atomic-pointer operation family. Current main now has
fixture-backed `atomic_pointer_state` classification for the observed
`fetch_and`, `fetch_or`, and `fetch_xor` shapes, and the focused
[crossbeam rerun](2026-05-26-crossbeam-atomic-pointer-rerun.md) now reports
those cards as `requires_loom`. Any new fixture should come from a future
still-missing atomic pointer/state shape.

### Layout, FFI, and platform routes

`mio-pr1388` is useful because it contains `zeroed`, raw pointer write/deref,
unsafe function call, and FFI cards in one platform-heavy PR. The one FFI card
is `miri_unsupported`, which is appropriate; the report must not imply a Miri
result exists.

## Possible false positives

- The original `crossbeam-pr1226` generic `unknown` cards were too broad for
  reviewers; the focused rerun moves them to atomic pointer/state route cards.
  Treat those as concurrency-review prompts, not proof that a model was run.
- `memchr-capped` capped inventory contains many `unknown` unsafe-fn owner cards;
  the focused [comment-plan follow-up](2026-05-26-memchr-unknown-comment-plan.md)
  keeps those cards out of inline comment candidates while preserving them in
  the advisory bundle.
- `arrayvec-pr137` can look worse by card count even though the upstream PR is a
  soundness-oriented change. Reviewer usefulness should be judged by actionability,
  not by raw card count.
- `mio-pr1388` platform/layout cards may need human review even when local
  syntax looks guard-like.

## Possible false negatives

- The sampled real-crate corpus still has no safe/no-unsafe target; the separate
  fixture-level [no-card control](2026-05-26-no-card-control.md) records the
  current false-positive smoke without becoming real-crate precision evidence.
- Crossbeam atomic-pointer state was not classified beyond `unknown` in this
  sampled PR. The focused rerun now covers the observed fetch operations as
  `atomic_pointer_state` `requires_loom` cards.
- UTF-8 unchecked validation was not sampled in this report; use
  `arrayvec-pr138` for that follow-up.
- `NonNull::new_unchecked` stale-pointer controls were not sampled here; use
  `hashbrown-pr667` or `memchr-capped` when that family changes.

## Fixture follow-ups

- Keep the no-card fixture control report linked from the dogfood README, and add
  a real-crate no-unsafe target only if a suitable corpus candidate appears.
- Keep the focused `crossbeam-pr1226` atomic pointer/state rerun linked as a
  regression report.
- Keep the focused `arrayvec-pr288` `Vec::set_len` rerun linked as initialized
  range regression evidence.
- Keep `target_feature_safety_docs` as the guardrail for `memchr-capped`; do not
  turn target-feature docs into availability evidence.

## Analyzer follow-ups

Subsequent swarm work has completed the initial evidence-applicability rail:

- the [evidence applicability model](../../analysis/evidence-applicability-model.md)
  is implementation-backed;
- same-target, open-branch, and stale-evidence helper contexts are now factored
  for the initial family sequence;
- `Vec::set_len`, `MaybeUninit`, `NonNull`, `get_unchecked`, UTF-8 unchecked
  conversion, `unwrap_unchecked`, and `transmute` / `transmute_copy` should now
  be treated as regression-pressure families rather than as unfinished helper
  scaffolding.

Remaining follow-ups:

1. Add one atomic pointer/state fixture/control only if future dogfood exposes a
   still-missing concrete shape.
2. Keep FFI/layout route wording human-review-heavy; the focused [mio FFI route wording follow-up](2026-05-26-mio-ffi-route-wording.md) now guards the `miri_unsupported` next-action wording.
3. Add new analyzer breadth only when a future dogfood report or fixture exposes
   a concrete missing evidence shape plus a false-positive control.

## Reproduction commands

The report used the existing manifest commands for the selected targets:

```bash
rtk cargo run --locked -p unsafe-review -- check --root target/dogfood-work/arrayvec-pr137-root --diff target/dogfood-work/arrayvec-pr137.raw.patch --format json --max-cards 40 --out target/dogfood-work/arrayvec-pr137.raw.head.json
rtk cargo run --locked -p unsafe-review -- check --root target/dogfood-work/arrayvec --diff target/dogfood-work/arrayvec-pr288.raw.diff --format json --max-cards 20 --out target/dogfood-work/arrayvec-pr288.after-encode-call-evidence.json
rtk cargo run --locked -p unsafe-review -- check --root target/dogfood-work/hashbrown --diff target/dogfood-work/hashbrown-pr693.raw.diff --format json --max-cards 30 --out target/dogfood-work/hashbrown-pr693.after-infallible-unwrap-evidence.json
rtk cargo run --locked -p unsafe-review -- repo --root target/dogfood-work/memchr --format json --max-cards 50 --out target/dogfood-work/memchr.unsafe-review.after-target-feature-contract-evidence.json
rtk cargo run --locked -p unsafe-review -- check --root target/dogfood-work/crossbeam-pr1226-root --diff target/dogfood-work/crossbeam-pr1226.raw.diff --format json --max-cards 40 --out target/dogfood-work/crossbeam-pr1226.strict-provenance.head.json
rtk cargo run --locked -p unsafe-review -- check --root target/dogfood-work/mio-pr1388-root --diff target/dogfood-work/mio-pr1388.raw.patch --format json --max-cards 60 --out target/dogfood-work/mio-pr1388.after-local-safety-colon.json
```

## Trust boundary

Static unsafe contract review only. This report does not prove memory safety,
UB-free status, Miri-clean status, site execution, policy readiness, precision,
or recall. It records reviewer-usefulness observations from selected local
dogfood outputs.
