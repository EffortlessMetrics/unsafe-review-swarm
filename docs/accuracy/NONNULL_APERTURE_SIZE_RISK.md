# `NonNull` aperture denominator: size-risk note

Status: fixture-pinned measurement, not calibrated.
Test: `analysis::pipeline::tests::nonnull_aperture_denominator_measures_same_pointer_evidence`
Measured on: swarm main `fd29aa67` plus the #2231 aperture slice.

## What was measured

Within the declared aperture — single-line `NonNull::new_unchecked(ident)`
with a plain identifier argument in the `nonnull_*` guard-shape fixtures —
every seam is enumerated source-first (analyzer output is never consulted for
the denominator), then each seam must produce exactly one `NonNullUnchecked`
card whose `non-null` evidence is correct for the same pointer.

Raw counts at measurement time:

- Denominator: 26 seams across 26 fixtures (one seam per fixture).
- Positive guard fixtures: 7 — evidence must be present and name the pointer.
- Negative controls: 19 — evidence must be absent (reassigned, shadowed,
  unrelated-pointer, post-check, comment-only, or non-returning guards).
- Result: 26/26 seams recalled with correct same-pointer evidence.
- An independent line-pattern recount (`NonNull::new_unchecked(ident)` with a
  plain identifier argument) reproduces the 26-seam denominator; 4 further
  `NonNull::new_unchecked(...)` lines with method-call arguments
  (`bucket.as_ptr()`, `ptr.cast::<u16>()`, ...) are outside the aperture by
  construction.

## Size risks (why this stays fixture-pinned)

1. Small n: 26 observations cannot support a calibrated rate. Per #2231, tens
   of positives alone are not statistical proof; no uncertainty interval is
   reported because none would be meaningful at this size.
2. Correlated samples: exactly one hand-built seam per fixture, all variants
   of one syntactic shape in one operation family (`NonNullUnchecked`) and one
   obligation (`non-null`). These are not independent observations.
3. Fixture-bound: the denominator is the pinned corpus, not unfamiliar code.
   It earns no global recall claim and says nothing about multiline,
   turbofish, UFCS, macro, or non-identifier-argument forms.
4. Positivity by naming convention: positive vs negative is decided by the
   fixture directory name (`not_guard` / `not_evidence`), not by independent
   adjudication. This is a regression lock on intended behavior, not a
   labeled measurement. The convention is only valid inside the `nonnull_*`
   guard-shape family, so the aperture is family-scoped: a combined-tree run
   showed the `mixed_source_roles` role-taxonomy fixture (#2227) contributes
   a `SAFETY`-comment-only production seam the name rule would mislabel
   positive while the analyzer correctly leaves its evidence missing. That
   seam stays pinned by #2227's inventory test. The family scope is declared
   in the test, not silent.

## What would reduce the risk

- An independently adjudicated seam denominator from #2224, labeled
  source-first before seeing analyzer output.
- Claim-specific criteria frozen before evaluation, with raw
  numerators/denominators, uncertainty, and correlated-sample limits (#2231).
- A larger, diverse sample spanning projects and syntactic shapes, with quiet
  controls and an explicit re-evaluation rule when family semantics change.

Until then the claim wording stays at the `fixture-pinned` tier:
"Fixture-backed for this pattern." No calibrated precision/recall,
no support-tier promotion, no policy eligibility.
