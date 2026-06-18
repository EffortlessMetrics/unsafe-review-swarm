# UNSAFE-REVIEW-SPEC-0042: corpus validation taxonomy

Status: proposed
Owner: repo-infra
Created: 2026-06-15

## Purpose

This spec defines the four-layer corpus validation taxonomy for unsafe-review.
It records what each validation layer proves, what it is blind to, when it runs,
and what artifacts it produces. It also establishes the claim boundaries that
apply to every layer.

Most layers already exist. This taxonomy formalizes and extends them. It is not
a second source of truth: the authoritative artifacts remain
`policy/detector-contracts.toml`, `policy/calibration.toml`,
`docs/dogfood/corpus.toml`, and `policy/spec-coverage.toml`. This spec is an
index and a discipline document, not a replacement for those ledgers.

## Taxonomy overview

The four layers answer different questions. They must not be collapsed:

```
detector controls : do detectors avoid known false-credit failure modes?   (exact)
pure examples      : is each evidence shape presented correctly per surface? (exact)
real-repo corpus   : does the tool behave on real unsafe-heavy code?         (invariants)
real-PR corpus     : does the PR experience stay useful and low-noise?       (movement)
```

A gap analysis (2026-06-15) confirmed that the first and third layers already
exist in the repository. The second layer is partial. The fourth layer is the
genuine new addition this lane delivers.

## Layer 1: Detector-control corpus

**Already exists** as `policy/detector-contracts.toml` and
`policy/calibration.toml`.

### Purpose

Prove that each detector avoids known false-credit failure modes for the
operation families it covers.

### What it proves

- Detectors fire on true call-site matches inside unsafe scope (D1).
- Detectors do not fire on function definitions or declarations (D2).
- Guards on a different receiver, pointer, or origin do not discharge the
  candidate site (D3).
- Detectors do not fire on commented-out or string-literal occurrences of the
  pattern (D4).
- Path-anchored detectors do not match on incidental tail-segment spelling (D5).
- Each operation family has at least one positive fixture and at least one
  negative control for each discipline check it must satisfy.

The 616 fixtures and 300+ negative controls (`_not_guard` / `_no_cards` suffix
naming) are enumerated in `policy/calibration.toml`. The per-family D1-D5
discipline contract entries live in `policy/detector-contracts.toml`.

### What it is blind to

Real-code over-match the fixture author never anticipated. A fixture suite
encodes the author's assumptions; it is blind to assumptions the author did not
know they were making. Patterns that appear in real code in forms the fixture
author never encoded are not covered here.

### Gate cadence

Every PR. Gates: `check-fixtures`, `check-calibration`, `check-detector-contracts`
(all part of `check-pr`).

### Artifacts

`policy/calibration.toml` — fixture-to-expected-cards map with class, operation
family, hazard, and support tier.
`policy/detector-contracts.toml` — per-family D1-D5 discipline contract with
negative-fixture coverage gaps tracked as documented exceptions.
Fixture directories under `fixtures/<name>/` — exact `expected.cards.json`
goldens for every registered fixture.

### Claim boundary

Exact goldens only for crafted fixture cases. Fixture calibration is
obligation-level evidence for specific detection shapes; it is not a global
precision or recall claim. This layer does not prove the tool is free of
false positives on code it has never seen.

---

## Layer 2: Pure-example corpus

**Today partial.** Only `raw_pointer_alignment` is fully exercised across all
user-facing surfaces via `crates/unsafe-review/tests/e2e.rs` and
`check-first-pr-artifacts`. The remaining fixtures are exercised only at the
cards level.

### Purpose

Prove that each evidence shape is presented correctly on every user-facing
output surface — not just `cards.json` but the full projection set.

### What it proves

For a representative set of exemplar fixtures spanning all operation families:
- `cards.json` content matches the calibration golden.
- `comment-plan.json` includes expected fields, selection reasons, and
  trust-boundary text.
- `pr-summary.md` renders the correct card counts and class distribution.
- `lsp.json` projects the expected diagnostics at the correct file/line.
- `repair-queue.json` lists the expected repair items with correct bucket
  assignments.
- `unsafe-review-gate.json` reflects the correct movement counts and status.
- `policy_report.json` / `policy_report.md` carry the correct policy posture.
- SARIF output is schema-valid and references the correct rule ids.

These fixtures are the committed examples used in documentation. Their goldens
are the normative source for "what does this evidence shape look like across
every surface."

### What it is blind to

Real-code variation. Pure examples are crafted to exercise specific shapes; they
do not exercise the full range of syntactic variation that appears in real Rust
codebases.

### Gate cadence

Every PR for the fixtures that have full-surface goldens (`check-pr` via
`check-fixture-surface-parity` and `check-surface-determinism`). New exemplar
fixtures join the every-PR path once their multi-surface goldens are committed
(PR-3 in the lane sequence).

### Artifacts

Per-exemplar fixture golden files: `expected.cards.json`,
`expected.comment-plan.json`, `expected.lsp.json`,
`expected.repair-queue.json`, `expected.unsafe-review-gate.json`, and surface
parity/determinism check output from `check-fixture-surface-parity` and
`check-surface-determinism` (introduced in PR-3).

### Claim boundary

Exact goldens are correct here. These are crafted, purpose-built cases with
known expected output. The gate fails if any surface diverges from the
committed golden. Exact goldens are NOT used for real-repo or real-PR corpora.

---

## Layer 3: Real-repo corpus

**Already exists** as `docs/dogfood/corpus.toml` pinning 12 repositories at
exact commit SHAs (37 targets). Today the manifest is validated by
`check-dogfood` but the corpus is never executed (no clone / run / invariant
check). Executing the corpus is PR-2 in the lane sequence.

### Purpose

Prove that the tool behaves correctly on real unsafe-heavy Rust code that the
detector authors did not write and did not anticipate. This is the check that
the fixture suite cannot supply.

### What it proves

Against pinned real-repo commits:
- No crash or panic on any target (no-crash invariant).
- All output artifacts are schema-valid JSON.
- Projection consistency: every surface projects from the same `ReviewCard` set.
- Card count stays within a tracked range per target (bounded, not exact).
- Known false-positive classes that were previously hardened are absent.
- The `unknown`-family percentage and `target_feature`-family percentage are
  recorded (diagnostic, not a threshold gate).
- Execution completes within the per-target time bound recorded in `corpus.toml`.

### What it is blind to

PR-diff movement. The real-repo corpus runs the tool in repo-scan mode against
pinned source trees, not against base/head PR diffs. It does not exercise the
outcome_movement, baseline, or comment-plan selection paths.

### Gate cadence

Nightly or release-readiness manual run. Never on every PR: zerocopy alone
scanned in 282s, making the corpus unacceptable as a PR gate. A separate
`check-corpus-backstop` advisory gate (SPEC-0039) provides a manifest-level
schema check every PR; the execution gate runs on a schedule.

### Artifacts

`docs/dogfood/corpus.toml` — pinned repo list at exact commit SHAs.
Per-target bounded invariant reports generated at runtime (not committed).
Execution is authenticated but read-only; no automatic third-party issue filing.

### Claim boundary

Real-repo and real-PR corpora are diagnostics, never proof. The corpus
explicitly records which known false-positive classes are absent; it does not
claim global false-positive freedom. Invariants and classifications are the
correct contract here — NOT exact every-card goldens — because pinned real code
still drifts conceptually between runs. No calibrated precision or recall claim
is made from corpus results. This layer does not establish that the tool is
UB-free, Miri-clean, or memory-safety-proof for any analyzed code.

---

## Layer 4: Real-PR corpus

**The genuinely new layer.** Today nearly absent. Introduced in PR-4 of the
corpus lane.

### Purpose

Prove that the PR experience stays useful and low-noise as the analyzer evolves.
Exercises paths the real-repo corpus cannot: outcome_movement, baseline
inheritance, comment-plan selection, and the full artifact bundle shape.

### What it proves

Against pinned base/head commit SHAs with checked-in diffs and expected counts:
- Movement shape: `new`, `worsened`, `resolved`, `inherited` counts stay within
  tolerance of the committed expected values.
- `no-new-debt` exit code matches expectation for the pinned diff.
- Comment-plan selection: expected cards are selected, expected cards are not
  selected, with recorded reason codes.
- Inherited quietness: a PR that inherits all pre-existing debt produces a
  clean no-new-debt result.
- Agent-readiness: agent-ready cards carry the expected `agent_readiness` field.
- Artifact-bundle shape: all expected artifact files are present and
  schema-valid.

### What it is blind to

Code the corpus did not include. The real-PR corpus proves the PR experience
for the pinned diffs it contains; it does not prove behavior on arbitrary new
PRs. New false-positive patterns in code not represented in the corpus are not
detected by this layer.

### Gate cadence

Release readiness or nightly. A subset of high-value, fast-running real-PR
corpus cases may join the every-PR path if they run in bounded time. The full
corpus never blocks every PR.

### Artifacts

`policy/pr-corpus.toml` — pinned base/head SHAs, checked-in diffs, expected
outcome_movement counts, and comment-plan selection expectations.
Per-case movement reports generated at runtime (not committed in full).

### Claim boundary

Same as the real-repo corpus: diagnostics, not proof. Movement counts are
recorded as toleranced expectations, not exact goldens. No calibrated
precision or recall claim. No UB-free, Miri-clean, or site-execution claim.

---

## External validation (informational, not a gate)

Running the tool on a real external PR read-only and classifying the output
provides adoption proof and surfaces friction the corpus layers cannot. This is
not an automated gate. No automatic third-party issue filing. Results are
recorded in `docs/dogfood/` as evidence entries. Output is classified into:
actionable, inherited, noisy, missed, agent-ready, human-only, cost, and
artifact-friction categories.

---

## Coverage map

This taxonomy EXTENDS the existing ledgers:

- `policy/spec-coverage.toml` — maps spec obligations to corpus cases and checks.
- `policy/stance-decisions.toml` — records each stance with fixtures and
  evidence links.
- `policy/detector-contracts.toml` — per-family D1-D5 coverage.

The coverage map is an index over those ledgers, not a parallel source of truth.
PR-5 of the corpus lane extends `stance-decisions.toml` entries with
`fixtures` / `dogfood_targets` / `surfaces` links and adds a
`check-stance-coverage` gate asserting that every stance has at least one
fixture and one piece of evidence.

The tie from spec obligation to corpus case to surface to check is:

```
spec obligation (SPEC-XXXX clause)
  -> stance-decisions.toml entry (owner-decided stance)
    -> fixtures/<name>/  (detector-control or pure-example layer)
    -> docs/dogfood/corpus.toml target (real-repo layer)
    -> policy/pr-corpus.toml case (real-PR layer)
      -> output surface (cards.json / comment-plan.json / lsp.json / ...)
        -> xtask gate (check-pr / check-fixture-surface-parity / check-surface-determinism / check-real-pr-corpus)
          -> documented exception (if coverage is partial)
```

---

## Claim boundary and trust boundary

These constraints apply to every layer and every output surface:

- unsafe-review does not **prove** code safe, memory-safe, or free of undefined
  behavior.
- unsafe-review does not claim **UB-free** or **Miri-clean** status for any
  analyzed site or corpus run.
- unsafe-review does not perform **site execution** or report witness execution
  results unless a separate witness receipt (from Miri, cargo-careful, Loom,
  Shuttle, or a named tool) is attached and imported via the receipt system.
- unsafe-review does not assert **calibrated precision or recall**. Fixture
  calibration is obligation-level evidence for specific detection shapes; it is
  not a global accuracy claim. Real-repo and real-PR corpus results are
  diagnostics, never global accuracy proof.
- Exact goldens are used **only** for crafted pure examples (layers 1 and 2).
  Real repos use invariants and classifications, not exact every-card goldens,
  because pinned real code still drifts conceptually between runs.
- Commits are **pinned at exact SHAs**. Floating branches are not permitted in
  corpus manifests.
- **No automatic third-party issue filing.** Corpus runs are read-only; any
  issue-filing from corpus results is a manual, deliberate action.
- **Single truth.** Extend `calibration.toml` / `corpus.toml` /
  `stance-decisions.toml` / `spec-coverage.toml` / `detector-contracts.toml`.
  Do not duplicate them or create a parallel ledger.
- The default analysis path remains syntax-first and build-free. No corpus run
  requires the analyzed repository to build successfully.
- No corpus result **blocks** merges or posts comments by default. Corpus
  execution is advisory infrastructure; it feeds evidence back into the ledgers.
- The ReviewCard is the single truth object. All surfaces project from it.

---

## Implementation tracking

This spec is implemented by the corpus-validation-system lane. The PR sequence is:

- PR-0 (landed): lane anchor — registered lane in `.rails/index.toml` +
  `.rails/goals/active.toml` and implementation plan.
- PR-1 (this spec): corpus taxonomy spec. Docs only. Status: proposed.
- PR-2: executable real-repo corpus. Adds `dogfood-exec` capability, seeds
  corpus.toml with nix / simdutf8 / zerocopy profiles, validates bounded
  invariants. Off the every-PR path.
- PR-3: pure-example multi-surface goldens. Commits `expected.comment-plan.json`
  / `expected.lsp.json` / `expected.repair-queue.json` for exemplar fixtures.
  Adds `check-fixture-surface-parity` and `check-surface-determinism`. Exact
  goldens; joins `check-pr`.
- PR-4: real-PR movement corpus. New `policy/pr-corpus.toml` with pinned
  base/head SHAs + checked-in diffs + expected outcome_movement counts.
- PR-5: coverage-map index. Extends `stance-decisions.toml` with fixture /
  dogfood-target / surface links; adds `check-stance-coverage`.

See `.rails/lanes/corpus/implementation-plan.md` for the full sequence and
evidence grounding.
