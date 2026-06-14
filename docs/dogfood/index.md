# Dogfood outcome index

Date: 2026-06-05
Status: experimental selected-corpus evidence
Source manifest: [`corpus.toml`](corpus.toml)
Machine-readable index: [`index.json`](index.json)
Usefulness notes: [`usefulness-notes.md`](usefulness-notes.md)
Bun stable-byte seeds: [`stable-byte-follow-up-seeds.md`](stable-byte-follow-up-seeds.md)

This index is a front panel for the real-crate dogfood corpus. It summarizes
which crates and PR diffs have been used to exercise `unsafe-review`, where the
saved-output artifacts are expected to live, and which outcome movement is
currently recorded.

## Trust Boundary

Dogfood corpus records are static unsafe contract review evidence. They are not
memory-safety proof, not UB-free status, not Miri-clean status, not a
site-execution claim unless an exact witness receipt is attached, and not
calibrated precision or recall.

Generated scan outputs are intentionally recorded as `local_untracked` artifacts
under `target/dogfood-work/`. Re-run the command in `corpus.toml` when a fresh
local artifact is needed.

## Corpus Summary

| Measure | Count |
|---|---:|
| Repositories | 12 |
| Total targets | 37 |
| Capped repo snapshots | 12 |
| PR diff targets | 23 |
| Fixture control targets | 2 |
| Checked-in scan outputs | 0 |

## Selected Judgment Sample

This sample counts committed real-crate reviewer judgment files only. It is a
repeatability denominator for manual usefulness rows, not a calibration
denominator, precision/recall claim, witness result, or policy gate. Fixture
controls, including manual-candidate smoke controls, stay outside this
real-crate sample.

| Measure | Count |
|---|---:|
| Real-crate targets | 6 |
| Judgment files | 6 |
| Card or surface judgments | 14 |
| Missed-obligation rows | 0 |

| Judgment label | Count |
|---|---:|
| `actionable` | 9 |
| `noise` | 2 |
| `missed` | 0 |
| `uncertain` | 1 |
| `human-only` | 2 |
| `good-agent-task` | 0 |
| `bad-agent-task` | 0 |

Selected real-crate targets:

- `arrayvec-pr138`
- `arrayvec-pr288`
- `crossbeam-pr1226`
- `hashbrown-pr667`
- `memchr-capped`
- `mio-pr1388`

## Repository Coverage

| Repository | Snapshot targets | PR diff targets | Primary exercise |
|---|---:|---:|---|
| `servo/rust-smallvec` | 1 | 4 | Raw pointer reads/writes, `Vec::set_len`, pointer arithmetic, unsafe impls, owner inference |
| `bluss/arrayvec` | 1 | 6 | `MaybeUninit`, `Vec::set_len`, raw pointer reads/writes, raw pointer accessor soundness fixes, UTF-8, drop/deallocation cards |
| `BurntSushi/memchr` | 1 | 1 | SIMD target-feature contracts, pointer arithmetic, unchecked constructors |
| `rust-lang/hashbrown` | 1 | 8 | Large-file syntax scanning, `MaybeUninit`, pointer arithmetic, unchecked/infallible operations, unsafe-call contract gaps, dedupe |
| `tokio-rs/bytes` | 1 | 1 | `Vec::from_raw_parts`, slice construction, ownership-transfer review cards |
| `crossbeam-rs/crossbeam` | 1 | 2 | Unsafe Send/Sync, atomics, raw pointer, ownership-transfer, strict-provenance Miri cfg cards, and atomic pointer state transitions |
| `tokio-rs/mio` | 1 | 1 | Unsafe function call contracts, `Vec::set_len`, zeroed values, pointer operations, socket address layout conversions, and unsafe Send/Sync route cards |
| `fitzgen/bumpalo` | 1 | 0 | Pointer arithmetic, slice construction, `str_from_utf8_unchecked`, unsafe fn call, and unsafe impl cards; fresh-crate capstone |
| `tokio-rs/slab` | 1 | 0 | Pointer arithmetic, unsafe fn call, and unsafe impl cards; fresh-crate capstone |
| `Lokathor/bytemuck` | 1 | 0 | `MaybeUninit` assume_init, unsafe fn call, unsafe impl, and raw pointer dereference cards; fresh-crate capstone |
| `matklad/once_cell` | 1 | 0 | Unsafe fn call, raw pointer dereference, and unsafe impl cards including witness-receipt-routing cases; fresh-crate capstone |
| `Amanieu/parking_lot` | 1 | 0 | Unsafe fn call, unsafe impl, raw pointer dereference, and pointer arithmetic cards; fresh-crate capstone |

## Recorded Outcome Movement

| Target | Judgment | Before | After | New | Resolved | Improved | Regressed | Unchanged | Proof action | Witness route state | Claim boundary | Notes |
|---|---|---|---|---:|---:|---:|---:|---:|---|---|---|---|
| `memchr-capped` target-feature contract evidence | `actionable` | `target/dogfood-work/memchr.unsafe-review.after-slice-end-pointer-evidence.json` | `target/dogfood-work/memchr.unsafe-review.after-target-feature-contract-evidence.json` | 0 | 0 | 10 | 0 | 40 | Keep the `target_feature` contract evidence as reviewability movement; attach an exact external witness receipt before treating the route as witnessed evidence. | `external-receipt-missing` | Static unsafe contract review saved-outcome movement only; not calibrated precision or recall, not witness execution, not site execution evidence, not policy readiness, not memory-safety proof, not UB-free status, not Miri-clean status, and not a site-execution claim unless a matching witness receipt says so. | Documented `#[target_feature]` declarations moved from `guard_missing` to `guarded_unwitnessed`; no target-feature availability, site execution, or soundness claim. |

## Target Groups

### Capped Repo Snapshots

- `smallvec-capped`
- `arrayvec-capped`
- `memchr-capped`
- `hashbrown-capped`
- `bytes-capped`
- `crossbeam-capped`
- `mio-capped`
- `bumpalo-capped`
- `slab-capped`
- `bytemuck-capped`
- `once_cell-capped`
- `parking_lot-capped`

### PR Diffs

- `memchr-pr215`
- `smallvec-pr407`
- `smallvec-pr277`
- `smallvec-pr64`
- `smallvec-pr254`
- `arrayvec-pr308`
- `arrayvec-pr137`
- `arrayvec-pr138`
- `arrayvec-pr187`
- `arrayvec-pr174`
- `arrayvec-pr288`
- `hashbrown-pr469`
- `hashbrown-pr501`
- `hashbrown-pr556`
- `hashbrown-pr657`
- `hashbrown-pr667`
- `hashbrown-pr692`
- `hashbrown-pr681`
- `hashbrown-pr693`
- `bytes-pr826`
- `crossbeam-pr1226`
- `crossbeam-pr1187`
- `mio-pr1388`

### Fixture Controls

- `safe-code-no-cards-control` - fixture-level quiet/no-card control linked to
  [2026-05-26 no-card fixture smoke](reports/2026-05-26-no-card-control.md).
  It is not real-crate precision evidence.
- `bun-manual-candidates-first-pr-smoke` - fixture-level manual-candidate
  projection control linked to
  [2026-06-03 Bun manual candidates first-pr smoke](reports/2026-06-03-bun-manual-candidates-first-pr-smoke.md).
  It is not Bun runtime evidence, analyzer discovery evidence, or real-crate
  precision evidence.

## Local Workflow

Validate the dogfood manifest:

```bash
rtk cargo run --locked -p xtask -- check-dogfood
```

Run one target by copying its command from [`corpus.toml`](corpus.toml). Compare
saved snapshots when an analyzer change is meant to improve card quality:

```bash
rtk cargo run --locked -p unsafe-review -- outcome \
  --before target/dogfood-work/before.json \
  --after target/dogfood-work/after.json \
  --format markdown \
  --out target/dogfood-work/outcome.md
```

Update this index only when the corpus manifest or recorded outcome evidence
changes. Do not use it to claim calibrated precision, safety, or policy
readiness.

For `fixture-control` targets, keep the target under `fixtures/` and describe
the control as fixture-level evidence. Do not count fixture controls as
real-crate coverage, calibrated precision, or safety evidence.

Bun stable-byte seeds live in
[`stable-byte-follow-up-seeds.md`](stable-byte-follow-up-seeds.md). They are
manual-candidate workflow seeds, not real-crate dogfood measurements and not
analyzer-discovered ReviewCards.

For `pr-diff` targets, make sure the target checkout under `root` matches the
saved diff's expected source tree. A zero-card result from checkout drift is not
dogfood evidence; record zero-card PR diffs only as explicit false-positive
controls. Exploratory zero-card results for unsupported unsafe-adjacent classes
belong in the handoff or objective-audit limitations instead of the active
corpus table. When saving a raw diff from GitHub, use
`rtk proxy gh pr diff ...` so RTK does not compact away `diff --git` headers.
