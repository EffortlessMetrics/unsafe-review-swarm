# 2026-05-26 post-burst analyzer audit

Status: swarm handoff
Scope: analyzer and calibration PRs #350-#396

This audit records the operation-family surface after the recent fixture-backed
analyzer burst. It is a lock-in document, not a support-tier promotion,
calibration report, policy-readiness claim, or release note.

Trust boundary:

- static advisory review only
- no witness execution by default
- no automatic comments
- no source edits
- no default blocking
- no safety, UB-free, Miri-clean, site-execution, or calibrated precision claim

## Scope

The burst covered three kinds of work:

- capacity/evidence calibration controls in #350-#355
- operation-family recognizers and route controls in #367-#392
- comment-only and stale-evidence false-positive controls in #393-#396

Docs, editor, coverage, and output-projection PRs in the same numeric window are
outside this analyzer-family audit except where they affect how cards are
projected. They do not change analyzer support posture.

Current validation posture:

- claims remain fixture-pinned and claim-scoped
- dogfood corpus exists, but most families below are not dogfood-measured
- no family below is policy-eligible from this audit alone

Subsequent swarm status:

- [Evidence applicability model](../analysis/evidence-applicability-model.md)
  now records the implementation-backed helper checkpoint.
- The initial helper sequence has landed for `unwrap_unchecked`, UTF-8
  unchecked conversion, `get_unchecked`, `NonNull::new_unchecked`,
  `MaybeUninit::assume_init`, `Vec::set_len`, and `transmute` /
  `transmute_copy`.
- This audit remains the snapshot of PRs #350-#396. Later helper PRs close the
  first applicability refactor rail, but they do not promote support tiers,
  calibration, policy readiness, or safety claims.
- Future analyzer changes should come from a new fixture or dogfood observation
  that exposes a still-missing stale, wrong-target, dominance, or value-domain
  shape.

## Family audit

| Family | Accepted evidence | Rejected controls | Missing controls / risk | Dogfood status | Next action |
|---|---|---|---|---|---|
| `Vec::reserve` / `try_reserve` / capacity evidence | Same-receiver `reserve` and `try_reserve` capacity evidence for capacity-related obligations. | Comment-return claims, stale receiver evidence, stale capacity binding evidence, and receiver freshness regressions. | Capacity evidence still needs operation-specific applicability: same owner, same initialized range, no receiver reassignment, and no cross-branch drift. | Not dogfood-measured after burst. | Dogfood `smallvec-pr277`, `smallvec-pr64`, `smallvec-pr254`, and `arrayvec-pr288`; then factor same-receiver/staleness helpers. |
| `Vec::set_len` initialized range | Capacity/shrink evidence and selected initialized-range controls from fixture history. | Comment-only initialization evidence, stale receiver evidence, and closed-observation patterns. | Loop-init dominance, partial initialization, field initialization, and start/end bound freshness need clearer shared modeling. | Not dogfood-measured after burst. | Audit `arrayvec-pr288` and `smallvec-pr277`; seed evidence-applicability helper for same receiver and initialized range. |
| `Vec::from_raw_parts` | Allocation/capacity/ownership-transfer cards remain advisory and route-heavy. | Comment-return controls reject prose-only ownership claims. | Allocator identity, capacity provenance, and ownership transfer often exceed local syntax. | Not dogfood-measured after burst. | Dogfood `bytes-pr826`; keep human-review route wording explicit. |
| `get_unchecked` / `get_unchecked_mut` | Direct `get(index)` probes, `if let` probes, and `match` probes can discharge same-index bounds evidence when they stay tied to the same target. | Comment-return controls and probe shapes that do not establish usable same-index evidence. | Shadowed index, wrong slice/receiver, stale index reassignment, macro expansion, and branch dominance need a shared applicability check. | Not dogfood-measured after burst. | Dogfood `arrayvec-pr137` and hashbrown targets; factor same-index/same-receiver checks after audit. |
| `str::from_utf8_unchecked` | `str::from_utf8` validation through `if let`, `match`, `let else`, and error-branch patterns. | Comment-only validation controls; closed or non-dominating observations must not count as validation. | Same-buffer matching, buffer reassignment, prefix/suffix validation, aliasing, and macro/cfg uncertainty need clearer model boundaries. | Not dogfood-measured after burst. | Dogfood `arrayvec-pr138`; factor same-buffer/staleness helper after one report. |
| `unwrap_unchecked` | `Option` and `Result` evidence through valid-value checks, `let else`, and `match` patterns. | Comment-return controls and stale branch guard controls. | Shadowed receiver, wrong result variable, cross-function invariants, and branch openness need shared applicability rules. | Not dogfood-measured after burst. | Dogfood `hashbrown-pr693`; factor same-receiver and open-branch checks before adding new shapes. |
| `NonNull::new_unchecked` | `NonNull::new` pattern and `match` guards can provide local nullability evidence when tied to the same pointer. | Wrong-pointer, stale-pointer, and provenance-uncertain shapes should remain carded or route-heavy. | Cast/provenance uncertainty, pointer reassignment, and macro-generated constructors need explicit controls. | Not dogfood-measured after burst. | Dogfood `hashbrown-pr667` and `memchr-capped`; add stale-pointer controls before breadth. |
| `MaybeUninit::assume_init` | Narrow same-slot write/new evidence and branch-write evidence for initialized-value obligations. | Comment-only initialization controls, other-slot writes, and closed-branch observations. | Partial initialization, field initialization, array/list initialization loops, and slot identity through destructuring need tighter modeling. | Not dogfood-measured after burst. | Dogfood `arrayvec-capped`, `hashbrown-pr692`, and `arrayvec-pr288`; factor same-slot and branch-openness checks. |
| FFI / extern / libc route | Same-file extern calls, libc calls, and extern owner-contract cards route to FFI-oriented review obligations. | Non-libc wrapper route controls prevent over-routing ordinary wrappers as libc/FFI seams. | Wrapper ownership, ABI/layout contracts, platform-specific behavior, and callback ownership remain human-review-heavy. | Not dogfood-measured after burst. | Dogfood `mio-pr1388`; keep route language advisory and operation-specific. |
| `unsafe impl Send` / `unsafe impl Sync` | Unsafe impls stay ReviewCard-backed with concurrency/witness route posture. | Custom unsafe impl route controls avoid misclassifying unrelated custom impls or wrapper contexts. | Trait bound evidence, interior mutability, ownership transfer, and concurrency interleavings are not locally proven. | Not dogfood-measured after burst. | Dogfood `crossbeam-pr1226` and `mio-pr1388`; route to Loom/Shuttle or human review without policy claims. |
| `#[target_feature]` / target-feature unsafe | Owner/caller documentation can provide contract evidence for target-feature cards. | Missing-doc cases and cfg predicates must not become target-feature availability proof. | Runtime dispatch proof, CPU feature availability, cfg/platform coverage, and callsite reachability remain outside static card proof. | One earlier `memchr-capped` outcome movement exists; not re-measured for this burst. | Re-run `memchr-capped` only when target-feature output changes; keep no site-execution wording. |
| `static mut` | Owner-contract cards route global mutable state to concurrency-oriented review. | Comment-only or nearby prose must not become synchronization evidence. | Alias discipline, interrupt/thread ownership, Loom/Shuttle feasibility, and platform single-thread assumptions need manual review. | Not dogfood-measured after burst. | Add dogfood target only if a real PR exposes actionable `static mut` cards. |
| `transmute` | Value-validity cards remain operation-specific and route-heavy. | Comment-only value controls reject prose-only validity claims. | Layout equivalence, enum niche validity, padding, and source/destination type identity need family-specific evidence rules. | Not dogfood-measured after burst. | Dogfood `mio-pr1388`; do not broaden beyond fixture-backed value classes. |

## Cross-family observations

The burst improved local syntactic evidence coverage, but many recognizers now
share the same hidden questions:

- Does the evidence target the same receiver, buffer, pointer, index, owner, or
  initialization slot as the unsafe operation?
- Does the evidence dominate the unsafe operation on an open branch?
- Was the relevant value reassigned or shadowed after the evidence?
- Is the evidence a real guard/contract, or only a comment or closed
  observation?
- Does macro, cfg, or wrapper context make the static identity uncertain?

These questions should become the evidence applicability model before new broad
recognizer families are added.

## Follow-up seeds

Immediate validation status:

1. Post-burst dogfood snapshot reporting is recorded in
   [2026-05-26 post-burst analyzer snapshot](../dogfood/reports/2026-05-26-post-burst.md).
2. The dogfood triage vocabulary is recorded in
   [dogfood triage taxonomy](../dogfood/triage-taxonomy.md).
3. Artifacts remain local/untracked unless a focused report explicitly records
   a checked-in output contract.

Architecture status:

1. The evidence applicability model is defined in `docs/analysis/`.
2. The first helper sequence is factored and fixture-backed.
3. The next analyzer work should be a small dogfood- or fixture-driven
   correction, not a broad recognizer expansion.

Do not do next:

- promote support tiers from this audit
- publish source from this audit
- claim calibration, precision, recall, safety, UB-free status, Miri-clean
  status, or policy readiness
- add new analyzer breadth before dogfood exposes the next specific gap
