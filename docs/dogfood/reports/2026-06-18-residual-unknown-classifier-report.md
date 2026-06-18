# Dogfood report: 2026-06-18 residual unknown classifier report

Status: residual classifier selection report
Swarm base commit: `c4d29b69`
Artifact status: local, untracked under `target/dogfood-work/` and `target/residual-unknown/`

This report records the residual `operation_family: "unknown"` shape after the
`unsafe_declaration` family split and the explicit comment-surfacing stance
landed. It is a classifier-selection report: the goal is to decide whether the
next analyzer family is large, cleanly separable, semantically coherent, and
measurable enough to implement.

The answer from this run is no new classifier PR yet. The largest residual
bucket is unsafe impls for domain traits, allocator traits, and other custom
contracts. That bucket is real, but it is the deferred unsafe-impl family lane
and needs its own stance/corpus evidence before implementation. The remaining
unsafe-block residuals are mixed syntax-light review fallbacks rather than one
clean operation family.

## Trust boundary

This is static unsafe contract review advisory evidence only. It is not a
support-tier promotion, calibration report, policy decision, safety proof,
not UB-free status, not a Miri result, not Miri-clean status, not a witness
result, not site-execution proof, not release readiness, and not a calibrated
precision or recall figure. No witness tools were run. The capped repo-snapshot
counts are selected corpus telemetry, not an ecosystem-wide measurement.

## Scope

Two evidence surfaces were used:

- the current capped repo-snapshot dogfood corpus through `dogfood-exec`;
- the committed fixture goldens plus first-pr comment-plan output for the four
  remaining fixture cards whose family is `unknown`.

The repo-snapshot corpus is capped at 50 cards per target by design. The
selected/not-selected counts below apply only to the four fixture first-pr
bundles, because repo-mode dogfood artifacts do not produce comment plans.

## Commands

```bash
rtk rg -n '"operation_family"[[:space:]]*:[[:space:]]*"unknown"' fixtures docs policy
rtk cargo run --locked -p unsafe-review -- first-pr --root fixtures/ffi_token_in_string_not_route --diff fixtures/ffi_token_in_string_not_route/change.diff --out-dir target/residual-unknown/ffi_token_in_string_not_route
rtk cargo run --locked -p unsafe-review -- first-pr --root fixtures/split_unsafe_block --diff fixtures/split_unsafe_block/change.diff --out-dir target/residual-unknown/split_unsafe_block
rtk cargo run --locked -p unsafe-review -- first-pr --root fixtures/unsafe_impl_custom_trait_not_send_sync --diff fixtures/unsafe_impl_custom_trait_not_send_sync/change.diff --out-dir target/residual-unknown/unsafe_impl_custom_trait_not_send_sync
rtk cargo run --locked -p unsafe-review -- first-pr --root fixtures/unsafe_impl_custom_trait_contract_not_guard --diff fixtures/unsafe_impl_custom_trait_contract_not_guard/change.diff --out-dir target/residual-unknown/unsafe_impl_custom_trait_contract_not_guard
rtk cargo run --locked -p xtask -- dogfood-exec --work-dir target/dogfood-work --max-cards 50 --strict --timeout 1200
```

Result:

- `dogfood-exec`: 15 ok / 0 failed.
- repo-snapshot cards: 711 capped cards.
- repo-snapshot residual unknown cards: 91.
- fixture residual unknown cards: 4.
- fixture comment-plan selection for unknown cards: 0 selected, 4 not selected.

## Repo-Snapshot Residuals

| Target | Cards | Unknown | Unsafe impl unknown | Unsafe block unknown |
|---|---:|---:|---:|---:|
| `smallvec-capped` | 50 | 0 | 0 | 0 |
| `arrayvec-capped` | 50 | 0 | 0 | 0 |
| `memchr-capped` | 50 | 1 | 0 | 1 |
| `hashbrown-capped` | 50 | 3 | 1 | 2 |
| `bytes-capped` | 50 | 5 | 5 | 0 |
| `crossbeam-capped` | 50 | 1 | 1 | 0 |
| `mio-capped` | 50 | 3 | 0 | 3 |
| `bumpalo-capped` | 50 | 2 | 2 | 0 |
| `slab-capped` | 11 | 0 | 0 | 0 |
| `bytemuck-capped` | 50 | 46 | 46 | 0 |
| `once_cell-capped` | 50 | 0 | 0 | 0 |
| `parking_lot-capped` | 50 | 9 | 7 | 2 |
| `nix-capped` | 50 | 13 | 1 | 12 |
| `simdutf8-capped` | 50 | 0 | 0 | 0 |
| `zerocopy-capped` | 50 | 8 | 3 | 5 |
| **Total** | **711** | **91** | **66** | **25** |

## Syntax Form Residuals

| Syntax form bucket | Count | Dominant examples | Classifier decision |
|---|---:|---|---|
| Unsafe impl domain trait | 56 | `Pod`, `Zeroable`, `AnyBitPattern`, `BufMut`, `RawMutex`, `ByteSlice`, `Validity`, `Cast`, `RegisterSet` | Large and coherent enough to study, but this is the deferred unsafe-impl family lane. Do not fold it into a generic unknown-reduction PR. |
| Unsafe block brace or block wrapper | 25 | `unsafe {`, `let res = unsafe {`, `const _: () = unsafe {` | Mixed syntax-light fallback bucket. Needs smaller fixture extraction before analyzer work. |
| Unsafe impl allocator trait | 6 | `GlobalAlloc`, `Allocator` | Same unsafe-impl lane; potentially a sub-bucket, not a standalone PR yet. |
| Unsafe impl Send/Sync-looking text | 2 | `Send`, `Sync` impl text that did not route to the existing Send/Sync family | Keep in unsafe-impl stance review; too small for a family PR by itself. |
| Unsafe impl other trait | 2 | Other custom trait impls | Same unsafe-impl lane. |

## Fixture Comment-Plan Residuals

| Fixture | Site kind | Class | Priority | Confidence | Selected | Not selected | Reason |
|---|---|---|---|---|---:|---:|---|
| `ffi_token_in_string_not_route` | `unsafe_block` | `guard_missing` | `high` | `medium` | 0 | 1 | `human_deep_review_only` |
| `split_unsafe_block` | `unsafe_block` | `contract_missing` | `high` | `high` | 0 | 1 | `human_deep_review_only` |
| `unsafe_impl_custom_trait_not_send_sync` | `unsafe_impl` | `guard_missing` | `high` | `medium` | 0 | 1 | `human_deep_review_only` |
| `unsafe_impl_custom_trait_contract_not_guard` | `unsafe_impl` | `guard_missing` | `high` | `medium` | 0 | 1 | `human_deep_review_only` |

These four cards stay visible as advisory ReviewCards and remain out of the
inline comment plan under the active stance.

## Decision

Do not start another classifier family from this report alone.

The only large, cleanly nameable residual bucket is unsafe impls. It is
semantically real, but unsafe impl classification was intentionally deferred
from the `unsafe_declaration` lane because it needs separate stance and corpus
evidence. A future unsafe-impl lane should decide whether the family needs one
canonical `unsafe_impl` label or narrower subfamilies such as allocator,
marker/domain-trait, raw-lock, and concurrency traits, and it must preserve
comment-plan eligibility as explicit surfacing policy.

The unsafe-block residuals are not one family. They include system-call
wrappers, string/comment-like controls, empty or split blocks, const blocks,
field access, and syntax contexts where the operation is not recoverable from
the current static surface. Those should stay `unknown` until a future report
finds a larger, fixture-backed, semantically coherent sub-bucket.

## Follow-up

No builder-ready classifier PR is filed from this report. The next useful
follow-up is an unsafe-impl stance/corpus evidence pass when the owner selects
that deferred lane. Until then, do not refine `unknown` merely to lower the
count.
