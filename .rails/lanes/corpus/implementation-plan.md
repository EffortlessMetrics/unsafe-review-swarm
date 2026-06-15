# Layered corpus validation system — implementation plan

## Lane thesis

The control-plane lane made detector discipline **declared and enforced**. This
lane makes that discipline **measured against evidence at every layer**, so the
question "is each evidence shape presented correctly, and does the tool behave on
code we did not author?" has a machine-checked answer instead of lore.

Validation has four distinct layers, each answering a different question. They
must not be collapsed — conflating them makes tests brittle or validation vague:

```
detector controls : do detectors avoid known false-credit failure modes?   (exact)
pure examples      : is each evidence shape presented correctly per surface? (exact)
real-repo corpus   : does the tool behave on real unsafe-heavy code?         (invariants)
real-PR corpus     : does the PR experience stay useful and low-noise?       (movement)
```

Evidence-grounded scope (2026-06-15): a fresh-crate dogfood (nix / simdutf8 /
zerocopy, 2492 cards) found **zero hard false positives** in the hardened
families — the detectors hold on unseen code. So this lane **locks validated-good
behavior**; it is not a detector-fix lane. The real friction it must surface is
volume and classifier coarseness (the `unknown`-family dominance, the card cap),
not over-matching.

Everything here is advisory-boundary-safe: no surface gains a calibrated
precision/recall, UB-free, Miri-clean, site-execution, or memory-safety-proof
claim. Real-repo and real-PR corpora are **diagnostics**, never proof.

## Reuse — do not reinvent (de-duplication is a hard rule)

A gap analysis (2026-06-15) confirmed most of the taxonomy already exists. Build
only the genuine gaps; do **not** create a ledger that duplicates an existing
source of truth.

- **Detector controls already exist**: `policy/detector-contracts.toml` +
  `policy/calibration.toml` (616 fixtures, 300+ negative controls). Gap:
  goldens are cards-only.
- **Real-repo corpus already exists**: `docs/dogfood/corpus.toml` pins 12 repos
  at **exact commit SHAs** (37 targets); `check-dogfood` validates the manifest.
  Gap: it is **never executed** (no clone / run / invariant check).
- **Pure examples are partial**: `crates/unsafe-review/tests/e2e.rs` +
  `check-first-pr-artifacts` already generate and validate all surfaces, but only
  `raw_pointer_alignment` is fully exercised.
- **Coverage map is partial**: `spec-coverage.toml` + `stance-decisions.toml` +
  `detector-contracts.toml` exist separately. Extend them with an index — do not
  add a parallel ledger.

The only genuinely new file this lane introduces is `policy/pr-corpus.toml` (the
real-PR movement layer).

## Sequence (one PR each; new gates born informational / off the every-PR path)

Ordering: taxonomy spec → execute the existing real-repo corpus → pure-example
surface goldens → real-PR corpus → coverage-map index. Heavy corpora
(real-repo/real-PR) run on a release/nightly/manual cadence, never blocking every
PR (zerocopy alone scanned in 282s). Only the deterministic exact-golden checks
(pure examples) join the `check-pr` bundle.

- **PR-0 (this PR) — lane anchor.** Register the lane in `.rails/index.toml` +
  `.rails/goals/active.toml` and land this plan. No code.
- **PR-1 — corpus taxonomy spec.** A spec defining the four layers + coverage
  map: purpose, what each proves, what it is blind to, gate cadence, the
  exact-goldens-only-for-pure-examples rule, and the claim boundaries (no
  calibrated/UB/real-repo-as-proof). Docs only.
- **PR-2 — executable real-repo corpus + dogfood seed.** Add a `dogfood-exec`
  capability that clones the pinned commits, runs the tool, and validates
  **bounded invariants** (no-crash, schema-valid, card-count within a range,
  recorded `unknown%` / `target_feature%`) — never exact every-card goldens. Seed
  `corpus.toml` with nix / simdutf8 / zerocopy (under-represented FFI-cfg /
  SIMD-target_feature / raw-pointer-transmute profiles). Off the every-PR path.
- **PR-3 — pure-example multi-surface goldens.** Commit
  `expected.comment-plan.json` / `expected.lsp.json` /
  `expected.repair-queue.json` (etc.) for ~5–10 exemplar fixtures spanning
  operation families; add `check-fixture-surface-parity` diffing every surface
  per exemplar. Exact goldens; may join `check-pr`.
- **PR-4 — real-PR movement corpus.** New `policy/pr-corpus.toml` with pinned
  base/head SHAs + checked-in diffs + expected `outcome_movement` counts +
  comment-plan selection expectations; a `check-real-pr-corpus` gate asserting
  movement shape within tolerance. Seed with a few high-signal PRs.
- **PR-5 — coverage-map index.** Extend `stance-decisions.toml` entries with
  `fixtures` / `dogfood_targets` / `surfaces` links; add `check-stance-coverage`
  (every stance has ≥1 fixture + evidence); optionally a surface-projection audit
  in `spec-coverage.toml`. Ties spec → stance → corpus → surfaces → check.

## Proof commands (per PR; this anchor PR runs the goals/pr subset)

```
cargo run --locked -p xtask -- check-goals
cargo run --locked -p xtask -- check-pr
cargo run --locked -p xtask -- source-divergence
git diff --check
```

## Boundaries

- Advisory only. Real-repo and real-PR corpora are diagnostics, not proof; no
  surface gains a calibrated precision/recall or UB/Miri/site-execution claim.
- Exact goldens **only** for crafted pure examples. Real repos use invariants and
  classifications (strict-but-correct kept separate from noisy), not exact
  every-card goldens, because pinned real code still drifts conceptually.
- Pin commits; never floating branches. No automatic third-party issue filing.
- Single truth: extend `calibration.toml` / `corpus.toml` /
  `stance-decisions.toml` / `spec-coverage.toml`; do not duplicate them.
- Out of scope: the `unknown`-family classifier refinement (an analyzer-breadth +
  stance change) rides **after** this lane, as the first change the corpus
  measures, so the reclassification can be proven non-regressive.
