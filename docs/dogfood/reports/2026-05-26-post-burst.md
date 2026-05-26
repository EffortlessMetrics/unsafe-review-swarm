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
| `arrayvec-pr288` | 8 | `vec_set_len`, `unsafe_fn_call` | `Vec::set_len` cards mostly moved to `guarded_unwitnessed`, which gives the reviewer a witness/action route instead of a generic missing-guard complaint. | One `try_push_str` `vec_set_len` card remains `guard_missing`; verify whether this is real initialized-range debt or missing loop/init recognition. | Best small target for initialized-range applicability work. |
| `hashbrown-pr693` | 15 | `unwrap_unchecked`, `unsafe_fn_call`, `nonnull_unchecked`, `raw_pointer_read` | `unwrap_unchecked` cards include `guarded_unwitnessed` local infallibility evidence, which is exactly the intended reviewer note shape. | Nearby unsafe-call and `NonNull` cards remain mixed with the unwrap cards; ranking/summary should keep the unwrap evidence easy to find. | Good target for same-receiver and open-branch checks. |
| `memchr-capped` | 50 | `unknown`, `unsafe_fn_call`, `target_feature`, `pointer_arithmetic` | `target_feature` cards remain `guarded_unwitnessed`, preserving the "contract exists, witness still absent" posture. | 24 `unknown` owner/unsafe-fn cards make the capped snapshot inventory-like. | Useful as a capped target-feature regression check, not as precision evidence. |
| `crossbeam-pr1226` | 6 | `unknown` | The cards point at changed atomic unsafe blocks in `fetch_and`, `fetch_or`, and `fetch_xor`. | All six are generic `unknown` `contract_missing`; they do not yet expose an atomic-pointer operation family or route. | Strong seed for atomic pointer/state operation modeling, not for broad analyzer support claims. |
| `mio-pr1388` | 18 | `unsafe_fn_call`, `zeroed`, `raw_pointer_deref`, `raw_pointer_write`, `ffi` | Layout/zeroed/raw-pointer cards point to concrete socket-address conversion review work. | The mix of `contract_missing`, `guard_missing`, and one `miri_unsupported` FFI card needs reviewer wording to avoid implying Miri coverage. | Good platform/layout target for human-review-heavy route wording. |

## Triage observations

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `arrayvec-pr288` | `vec_set_len` `try_push_str` card | `needs-fixture` | One remaining `guard_missing` may be real initialized-range debt or a missed same-vector initialization shape. | Add a focused fixture only after manual review confirms the dogfood shape. |
| `crossbeam-pr1226` | changed atomic unsafe blocks | `needs-analyzer` | Six cards are generic `unknown` despite the changed blocks being atomic pointer/state operations. | Continue atomic pointer/state classification with fixture-backed controls; keep concurrency proof out of scope. |
| `memchr-capped` | `unknown` unsafe-fn owner cards | `noise` | Twenty-four `unknown` cards make the capped run inventory-like rather than PR-review focused. | Use this target for capped regression scans and ranking pressure, not precision claims. |
| `memchr-capped` | `target_feature` | `actionable` | Target-feature cards preserve contract evidence while leaving witness evidence absent. | Keep `target_feature_safety_docs` as the guardrail; do not turn docs into availability proof. |
| `mio-pr1388` | FFI/platform layout cluster | `needs-route` | The FFI card is correctly `miri_unsupported`, but surrounding layout cards need human-review-heavy wording. | Improve route wording only with a focused fixture or projection rail. |
| selected corpus | safe/no-unsafe control | `needs-fixture` | The sampled corpus has no explicit zero-card/no-unsafe control target. | Add a no-unsafe dogfood control target or separate false-positive control report. |
| `arrayvec-pr137` | soundness-fix card count | `needs-doc` | The PR can look worse by card count even when it is a soundness-oriented upstream change. | Keep docs/reports centered on actionability rather than raw gap deltas. |

## Findings by family

### `Vec::set_len`

`arrayvec-pr288` is the most useful target. It produced seven `vec_set_len`
cards: most are `guarded_unwitnessed`, with one remaining `guard_missing`
around `try_push_str`. That is a good next audit point because it is narrow:
either initialized-range evidence is truly missing, or the recognizer needs a
same-buffer/loop-init control.

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

`memchr-capped` and `crossbeam-pr1226` show the main post-burst noise pocket.
`memchr-capped` has broad owner/unsafe-fn cards that are useful as inventory,
while `crossbeam-pr1226` exposes changed atomic unsafe blocks without a specific
atomic-pointer operation family. The next work should be better operation
classification and route wording, not a support claim.

### Layout, FFI, and platform routes

`mio-pr1388` is useful because it contains `zeroed`, raw pointer write/deref,
unsafe function call, and FFI cards in one platform-heavy PR. The one FFI card
is `miri_unsupported`, which is appropriate; the report must not imply a Miri
result exists.

## Possible false positives

- `crossbeam-pr1226` generic `unknown` cards may be too broad for reviewers
  unless atomic pointer/state classification improves.
- `memchr-capped` capped inventory contains many `unknown` unsafe-fn owner cards;
  use it for regression scans, not precision claims.
- `arrayvec-pr137` can look worse by card count even though the upstream PR is a
  soundness-oriented change. Reviewer usefulness should be judged by actionability,
  not by raw card count.
- `mio-pr1388` platform/layout cards may need human review even when local
  syntax looks guard-like.

## Possible false negatives

- No safe/no-unsafe dogfood control exists in the corpus yet.
- Crossbeam atomic-pointer state is not classified beyond `unknown` in this
  sampled PR.
- UTF-8 unchecked validation was not sampled in this report; use
  `arrayvec-pr138` for that follow-up.
- `NonNull::new_unchecked` stale-pointer controls were not sampled here; use
  `hashbrown-pr667` or `memchr-capped` when that family changes.

## Fixture follow-ups

- Add a no-unsafe dogfood control target or separate false-positive control
  report.
- Add or verify an atomic pointer/state fixture from the `crossbeam-pr1226`
  shape.
- Add a `Vec::set_len` initialized-range fixture for the `arrayvec-pr288`
  `try_push_str` shape if manual review says the remaining `guard_missing` is
  noisy.
- Keep `target_feature_safety_docs` as the guardrail for `memchr-capped`; do not
  turn target-feature docs into availability evidence.

## Analyzer follow-ups

1. Define the evidence applicability model before adding new recognizers.
2. Factor same-receiver/staleness helpers for `unwrap_unchecked`.
3. Factor same-buffer/staleness helpers for UTF-8 validation.
4. Improve atomic pointer/state classification from generic `unknown` cards.
5. Keep FFI/layout route wording human-review-heavy.

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
