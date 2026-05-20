# Dogfood outcome index

Date: 2026-05-18
Status: experimental selected-corpus evidence
Source manifest: [`corpus.toml`](corpus.toml)
Machine-readable index: [`index.json`](index.json)

This index is a front panel for the real-crate dogfood corpus. It summarizes
which crates and PR diffs have been used to exercise `unsafe-review`, where the
saved-output artifacts are expected to live, and which outcome movement is
currently recorded.

## Trust Boundary

Dogfood corpus records are static unsafe contract review evidence. They are not
a proof of memory safety, not UB-free status, not a Miri result unless an exact
witness receipt is attached, and not calibrated precision or recall.

Generated scan outputs are intentionally recorded as `local_untracked` artifacts
under `target/dogfood-work/`. Re-run the command in `corpus.toml` when a fresh
local artifact is needed.

## Corpus Summary

| Measure | Count |
|---|---:|
| Repositories | 7 |
| Total targets | 30 |
| Capped repo snapshots | 7 |
| PR diff targets | 23 |
| Checked-in scan outputs | 0 |

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

## Recorded Outcome Movement

| Target | Before | After | New | Resolved | Improved | Regressed | Unchanged | Notes |
|---|---|---|---:|---:|---:|---:|---:|---|
| `memchr-capped` target-feature contract evidence | `target/dogfood-work/memchr.unsafe-review.after-slice-end-pointer-evidence.json` | `target/dogfood-work/memchr.unsafe-review.after-target-feature-contract-evidence.json` | 0 | 0 | 10 | 0 | 40 | Documented `#[target_feature]` declarations moved from `guard_missing` to `guarded_unwitnessed`; no target-feature availability, site execution, or soundness claim. |

## Target Groups

### Capped Repo Snapshots

- `smallvec-capped`
- `arrayvec-capped`
- `memchr-capped`
- `hashbrown-capped`
- `bytes-capped`
- `crossbeam-capped`
- `mio-capped`

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

For `pr-diff` targets, make sure the target checkout under `root` matches the
saved diff's expected source tree. A zero-card result from checkout drift is not
dogfood evidence; record zero-card PR diffs only as explicit false-positive
controls. Exploratory zero-card results for unsupported unsafe-adjacent classes
belong in the handoff or objective-audit limitations instead of the active
corpus table. When saving a raw diff from GitHub, use
`rtk proxy gh pr diff ...` so RTK does not compact away `diff --git` headers.
