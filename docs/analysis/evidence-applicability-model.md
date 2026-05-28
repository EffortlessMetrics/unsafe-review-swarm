# Evidence applicability model

Status: implementation-backed design rail
Owner: analyzer / evidence
Created: 2026-05-26

This note defines the shared model for deciding when local code evidence applies
to a specific unsafe operation and obligation. It is both a design rail and the
current helper checkpoint for the implemented applicability refactors. It does
not promote support tiers, calibration, policy readiness, or safety claims by
itself.

Linked evidence:

- [Post-burst analyzer audit](../handoffs/2026-05-26-post-burst-analyzer-audit.md)
- [Post-burst dogfood snapshot](../dogfood/reports/2026-05-26-post-burst.md)
- [Accuracy validation and calibration](../accuracy/README.md)

Trust boundary:

- static advisory review only
- no witness execution by default
- no automatic comments
- no source edits
- no default blocking
- no safety, UB-free, Miri-clean, site-execution, precision, or recall claim

## Purpose

Recent fixture-backed recognizers improved many operation families, but the
families now repeat the same applicability questions:

```text
operation detected
-> obligation selected
-> evidence discovered
-> evidence target identified
-> applicability checked
-> obligation discharged or left missing
-> ReviewCard projected
```

The goal is to make those checks explicit before adding more analyzer breadth.
The analyzer should reject plausible-looking evidence when it targets the wrong
value, appears on a closed branch, is stale after reassignment, or is only prose
where executable guard evidence is required.

## Vocabulary

| Term | Meaning |
|---|---|
| Operation | The unsafe-adjacent expression or declaration that creates the ReviewCard site, such as `unwrap_unchecked`, `Vec::set_len`, `str::from_utf8_unchecked`, or `NonNull::new_unchecked`. |
| Obligation | The safety condition selected for that operation, such as initialized range, UTF-8 validity, non-null pointer, same allocation, or valid value. |
| Evidence candidate | A nearby code, contract, test, receipt, or route signal that might satisfy or inform an obligation. |
| Evidence target | The program value the candidate is about: receiver, buffer, pointer, index, initialization slot, owner, or feature-gated function. |
| Applicability | The answer to "does this candidate actually apply to this operation and obligation?" |
| Discharge evidence | Executable or structural evidence that can mark one obligation as present for this operation. |
| Contract evidence | Caller/maintainer prose that can satisfy contract-documentation obligations, but must not become executable guard evidence. |
| Witness evidence | Imported receipt evidence for an exact ReviewCard identity. Suggested witness routes are not witness evidence. |
| Reach evidence | Static test/path mentions or other reachability hints. These are review hints, not site-execution proof. |

## Applicability checks

Each obligation-specific recognizer should answer these questions in order.

### 1. Same target

Evidence must refer to the same program target as the operation:

| Target kind | Examples |
|---|---|
| Same receiver | `self`, `vec`, or `slice` used by `set_len`, `get_unchecked`, or an unsafe call. |
| Same buffer | The byte slice validated by `str::from_utf8` before `from_utf8_unchecked`. |
| Same pointer | The pointer checked for nullability, alignment, or Box-origin evidence. |
| Same index | The index probed with `get(index)` before `get_unchecked(index)`. |
| Same initialization slot | The `MaybeUninit` slot written before `assume_init`. |
| Same owner | The unsafe function, extern declaration, target-feature function, or impl whose contract is being reviewed. |

If target identity is uncertain because of macros, cfg, wrapper layers, casts, or
aliasing, keep the obligation missing or route it to human review instead of
silently discharging it.

### 2. Dominance and branch openness

Executable guard evidence must be on a path that reaches the operation.

Good shapes:

- guard before operation,
- enclosing positive branch,
- early-return rejection branch,
- `let else` that returns on failure,
- match arm where the operation is inside the validated arm.

Rejected shapes:

- post-operation checks,
- closed observation branches,
- one-sided observations that do not dominate the operation,
- unrelated branches that validate a different value.

### 3. Freshness

Evidence becomes stale when the relevant target changes before the unsafe
operation.

Freshness hazards:

- receiver reassignment,
- pointer reassignment,
- buffer reassignment,
- index mutation,
- capacity/length changes after the guard,
- shadowed bindings with the same name,
- wrapper calls that may change the target state.

If freshness cannot be established locally, leave the obligation missing.

### 4. Evidence kind

The analyzer must not mix evidence kinds:

| Evidence kind | Can discharge guard obligation? | Can discharge contract obligation? |
|---|---:|---:|
| Executable guard/assertion | yes, when applicable | no |
| `# Safety` docs / `SAFETY:` prose | no | yes, when scoped to the owner/site |
| Related test mention | no | no |
| Suggested witness route | no | no |
| Imported witness receipt | only witness evidence, exact-card scoped | no |
| Comment-only "returns if invalid" text | no | maybe contract evidence if scoped; never guard evidence |

## Family mapping

| Family | Evidence target | Applicability focus | Common false-positive control |
|---|---|---|---|
| `unwrap_unchecked` | Same `Option`/`Result` receiver or infallible result value | Valid state dominates operation and remains fresh. | Wrong receiver, stale guard, post-check, comment-only early-return text. |
| `str::from_utf8_unchecked` | Same byte buffer | UTF-8 validation dominates unchecked conversion and buffer remains fresh. | Wrong buffer, stale buffer, observed-only validation, post-validation. |
| `get_unchecked` / `get_unchecked_mut` | Same receiver and same index | Bounds probe or length guard applies to the same index and receiver. | Other receiver, stale receiver, stale index, post-check, closed branch. |
| `NonNull::new_unchecked` | Same pointer | Nullability guard applies to the pointer passed to unchecked constructor. | Wrong pointer, stale pointer, non-returning `is_null`, provenance uncertainty. |
| `MaybeUninit::assume_init` | Same initialization slot | Write/new evidence reaches the same slot before assume-init. | Other slot, stale write, partial init, closed branch, comment-only init. |
| `Vec::set_len` | Same receiver and initialized range | Initialization/capacity/shrink evidence applies to the new length. | Capacity-only evidence, unrelated const-CAP buffers, stale receiver, stale with_capacity receiver or length, partial-slice initialization, single-index initialization, unrelated initialization, comment-only init. |
| `Vec::from_raw_parts` | Same raw parts and ownership origin | Capacity/allocation/ownership evidence applies to the transferred parts. | Wrong capacity, stale pointer or cap, comment-only ownership, unknown allocator provenance. |
| `slice::from_raw_parts_mut` | Same raw parts and slice element context | MaybeUninit initialized-memory evidence applies only through the call arguments or return type that owns the raw-parts call. | Unrelated MaybeUninit local, unrelated function context, comment-only initialization. |
| `copy_nonoverlapping` / `ptr::copy` | Same source slice, destination slice, and count | Source and destination range evidence must both apply to the copy count; `copy_nonoverlapping` still needs separate non-overlap evidence. | Source-only or destination-only bounds, wrong length, stale source/count, closed branch, comment-only return. |
| Raw pointer read/write/arithmetic | Same pointer or pointer origin | Bounds, alignment, nullability, initialized, and allocation evidence apply to the accessed pointer. | Other pointer, shadowed origin, stale origin, align-only where bounds are needed. |
| Unsafe function call | Same callee and argument/receiver | Callee preconditions are known and argument evidence is tied to the call. | Local wrapper mistaken for FFI, wrong receiver, closed branch. |
| `new_unchecked` constructors | Same receiver type | Availability assertions, open branches, or early returns must target the receiver type being constructed. | Other receiver, observed-only availability, closed availability branch. |
| FFI / extern | Same foreign owner or call boundary | ABI/layout/lifetime/ownership contracts are present or missing for that boundary. | Non-libc wrapper over-routing, treating C boundary as Miri-ready. |
| `unsafe impl Send` / `Sync` | Same impl owner and type parameters | Concurrency invariants route to review/witness plan; local syntax does not prove them. | Custom trait impl misroute, missing bound context, treating Loom route as receipt. |
| `#[target_feature]` | Same annotated function/caller contract | Contract docs inform review; availability and dispatch remain unproven. | Cfg predicate treated as runtime availability, docs treated as witness. |
| `static mut` | Same static owner | Alias/synchronization contracts route to concurrency review. | Nearby prose treated as synchronization guard. |
| `transmute` / `zeroed` | Same source/destination type and value | Layout evidence must dominate the operation; valid-value/valid-zero evidence is type-specific and must keep the source value fresh after the guard. | Layout-only prose, observed layout equality, closed-branch layout assertions, comment-only value claim, stale byte/value check. |
| `unreachable_unchecked` | Same control-flow path | Infallible-path evidence applies only while the same match arm remains open at the unchecked call. | Other match context, post-operation evidence, closed infallible match. |

## Helper extraction order

Do not introduce a large generic engine in one PR. Extract helpers only when a
family already has fixture and dogfood pressure.

Current implementation checkpoint:

| Family | Current helper/context | Status | Next useful pressure |
|---|---|---|---|
| `unwrap_unchecked` | Receiver/state applicability helpers in the analyzer evidence path | factored | Dogfood `hashbrown-pr693` only when a new stale or wrong-receiver shape appears. |
| `str::from_utf8_unchecked` | Same-buffer UTF-8 validation applicability helpers | factored | Add prefix/suffix or alias controls only when fixture or dogfood evidence exposes them. |
| `get_unchecked` / `get_unchecked_mut` | `GetUncheckedBoundsApplicability` for same receiver/index, top-level conjunctive open branches, early returns, and stale targets; disjunctive branches remain non-discharge controls | factored | Dogfood `arrayvec-pr137` or hashbrown targets before adding new probe shapes. |
| `NonNull::new_unchecked` | `NonNullPointerContext` for same-pointer probes, top-level conjunctive open branches, early returns, and stale pointer checks; disjunctive branches remain non-discharge controls | factored | Add cast/provenance or macro controls only from concrete fixtures. |
| `MaybeUninit::assume_init` | `MaybeUninitSlotContext` for same-slot writes/new bindings, scope reach, and stale slot checks | factored | Partial-field and partial-array initialization are fixture-pinned as non-discharge evidence; add broader field-pattern recognition only as separate fixture-backed slices. |
| `Vec::set_len` | `SetLenApplicabilityContext` delegates capacity checks to `SetLenCapacityContext`, initialized-range checks to `SetLenInitializedRangeContext`, and call-result initialization checks to `SetLenCallResultInitializationContext` | factored | Existing fixtures pin unrelated const-CAP buffers, stale `with_capacity` receiver/new-length, stale reserve/new-length, and wrong-target initialized-range controls; keep using `arrayvec-pr288` as regression pressure for new capacity, initialized-range, and call-result shapes. |
| `Vec::from_raw_parts` | `VecFromRawPartsCallContext` ties pointer, len, cap, same-origin ManuallyDrop evidence, and pre-call len/cap guards back to the same call | factored | Existing fixtures pin len/cap guards, stale cap, stale pointer origin, closed branches, comment-only returns, and same-origin pointer/capacity/ownership evidence. |
| `slice::from_raw_parts_mut` | `SliceFromRawPartsContext` ties MaybeUninit initialized-memory evidence to the raw-parts call arguments or owning return type | factored | Existing fixtures pin MaybeUninit slice evidence and unrelated MaybeUninit locals as non-discharge controls. |
| `transmute` / `transmute_copy` | `TransmuteLayoutContext` and `TransmuteValueDomainContext` separate layout-size evidence from value-domain evidence; layout checks reject observed equality and closed-branch assertions, and the value-domain context owns same-source-value and stale-reassignment checks | factored | Do not broaden valid-value domains without one positive and one false-positive control. |
| `zeroed` | `ZeroedTargetContext` ties valid-zero evidence to the target type of the zeroed call | factored | Existing fixtures pin known primitive target types and `NonNull` as missing valid-zero evidence. |
| `copy_nonoverlapping` / `ptr::copy` | `CopyRangeApplicability` and `SliceCountBoundTarget` require same source slice, destination slice, and count evidence before discharging valid-range | factored | Existing fixtures pin source-only, destination-only, wrong-length, stale source/count, closed-branch, disjunctive-branch, and comment-only controls; add non-overlap or initialization breadth only as separate fixture-backed slices. |
| `write_bytes` | `WriteBytesCallContext` ties MaybeUninit, pointer-type, byte-value, and bounds evidence to the same receiver/value/count tuple | factored | Existing fixtures pin raw pointer write classification and valid-zero value evidence; add stale receiver/value controls only from concrete fixture pressure. |
| `unreachable_unchecked` | `UnreachableUncheckedPathContext` ties infallible-path evidence to the same still-open match context as the unchecked call | factored | Existing fixtures pin wrong-match, post-operation, and closed-match false-positive controls. |
| `new_unchecked` constructors | `UncheckedConstructorAvailabilityContext` ties availability evidence to the same constructor receiver type before the call | factored | Existing fixtures pin same receiver, other receiver, assert guards, unavailable early returns, observed-only availability, and closed branches. |
| `encode_utf8` unsafe call | `EncodeUtf8CapacityContext` ties the unsafe call to the remaining-capacity binding passed as its length argument | factored | Existing fixtures pin the remaining-capacity call shape; use arrayvec dogfood for wording pressure, not broad UTF-8 unchecked detection. |

Route-heavy implementation checkpoint:

These families are intentionally tracked as route/contract applicability rails,
not as unfinished local-discharge helper backlog. They should grow only when a
concrete fixture or dogfood report exposes a wrong-target, stale, dominance, or
wording gap.

| Family | Current rail | Status | Next useful pressure |
|---|---|---|---|
| Raw pointer read/write/arithmetic | ReviewCard hazards keep bounds, alignment, nullability, initialized-memory, and allocation obligations separate; no single nearby guard is treated as broad pointer safety proof. | route-heavy | Add same-origin or stale-origin controls only from concrete raw-pointer fixtures or dogfood cards. |
| Unsafe function call | Callee/receiver preconditions stay tied to the call site and wrapper context; generic unsafe calls route to human review unless a family-specific helper owns the obligation. | route-heavy | Add argument-target helpers only for one named callee family at a time. |
| FFI / extern | FFI cards keep ABI, layout, lifetime, and ownership obligations attached to the same foreign boundary or owner; sanitizer and cargo-careful remain suggested routes, not receipts. | route-heavy | Use `mio-pr1388` or another concrete boundary card before changing route wording or wrapper ownership handling. |
| `unsafe impl Send` / `Sync` | Concurrency cards stay tied to the same impl owner and type parameters; Loom/Shuttle are route suggestions and never concurrency proof. | route-heavy | Use `crossbeam-pr1226` only when a fixture can pin a specific state/owner distinction. |
| `#[target_feature]` | Owner/caller docs may be contract evidence for the annotated function; cfg predicates, docs, and witness routes are not runtime availability or site-execution proof. | route-heavy | Use memchr dogfood only for concrete contract, ranking, or wording drift. |
| `static mut` | Global mutable-state cards stay tied to the same static owner and route aliasing/synchronization obligations to review. | route-heavy | Add a dogfood target only if a real PR exposes actionable `static mut` review cards. |

Original extraction sequence, retained as the preferred order for auditing or
extending these families:

1. `unwrap_unchecked`: same receiver, open branch, stale guard.
2. `str::from_utf8_unchecked`: same buffer, stale buffer, validation branch.
3. `get_unchecked`: same receiver and index.
4. `NonNull::new_unchecked`: same pointer and stale pointer.
5. `MaybeUninit::assume_init`: same slot and branch openness.
6. `Vec::set_len`: same receiver, initialized range, and freshness.

Each helper PR should include:

- one behavior-preserving refactor or one fixture-backed behavior change,
- at least one accepted-evidence fixture,
- at least one rejected-control fixture,
- dogfood note if a real target exposed the gap,
- no support-tier promotion unless the calibration lane separately justifies it.

## Projection rule

`ReviewCard` remains the source of truth. Applicability changes should affect
obligation evidence on the card, then every downstream surface should project
from that card:

- human output,
- JSON,
- PR summary,
- comment plan,
- witness plan,
- SARIF,
- saved LSP,
- agent packet.

Do not add separate analyzer truth to output projections.

## Done criteria

This model is implementation-backed for the initial helper sequence:

- `unwrap_unchecked` receiver/state applicability,
- `str::from_utf8_unchecked` same-buffer validation applicability,
- `get_unchecked` / `get_unchecked_mut` bounds applicability,
- `NonNull::new_unchecked` pointer applicability,
- `MaybeUninit::assume_init` slot applicability,
- `Vec::set_len` range applicability,
- `transmute` / `transmute_copy` layout and value-domain applicability.

The rail remains advisory and maintenance-scoped. Future applicability work must
still satisfy:

- one family and one evidence shape per PR,
- accepted and rejected fixture coverage for behavior changes,
- dogfood notes where real-crate behavior changes reviewer usefulness,
- `check-pr`, `check-calibration`, and `check-dogfood` remain green,
- support-tier wording still avoids calibration, policy-readiness, and safety
  claims unless a separate support-tier promotion justifies them.
