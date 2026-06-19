# UNSAFE-REVIEW-SPEC-0042: corpus validation taxonomy

Status: proposed
Owner: repo-infra
Created: 2026-06-15

## Purpose

This spec defines the four-layer corpus validation taxonomy for unsafe-review.
It records what each validation layer proves, what it is blind to, when it runs,
and what artifacts it produces. It also establishes the claim boundaries that
apply to every layer.

This spec also defines the post-0.3.8 generalization overlay: how the same
corpus layers are partitioned into conformance, regression, and holdout use;
how evidence-loss challenge cases are recorded; and how external usefulness
pilots feed the backlog without becoming accuracy claims.

Most layers already exist. This taxonomy formalizes and extends them. It is not
a second source of truth: the authoritative artifacts remain
`policy/detector-contracts.toml`, `fixtures/calibration.toml`,
`docs/dogfood/corpus.toml`, and `policy/spec-coverage.toml`. This spec is an
index and a discipline document, not a replacement for those ledgers.

## Taxonomy overview

The four layers answer different questions. They must not be collapsed:

```text
detector controls : do detectors avoid known false-credit failure modes?   (exact)
pure examples      : is each evidence shape presented correctly per surface? (exact)
real-repo corpus   : does the tool behave on real unsafe-heavy code?         (invariants)
real-PR corpus     : does the PR experience stay useful and low-noise?       (movement)
```

A gap analysis (2026-06-15) confirmed that the first and third layers already
exist in the repository. The second layer is partial. The fourth layer is the
genuine new addition this lane delivers.

## Generalization partitions

Corpus partitions are an overlay on the four validation layers, not a fifth
layer and not a duplicate manifest. Partition metadata belongs next to the
existing source of truth for the case it classifies:

- fixture and pure-example cases: `fixtures/calibration.toml` and fixture
  goldens;
- real-repo cases: `docs/dogfood/corpus.toml`;
- real-PR cases: `policy/pr-corpus.toml` or the future external PR manifest
  when those cases are promoted from pilot evidence.

The enforced partition metadata is intentionally small and lives in those
ledgers:

```toml
# fixtures/calibration.toml
partition_default = "conformance"

# docs/dogfood/corpus.toml
partition_by_kind = { "fixture-control" = "conformance", "repo-snapshot" = "regression", "pr-diff" = "regression" }

# policy/pr-corpus.toml
partition_by_kind = { "synthetic-fixture" = "conformance" }
```

`xtask check-corpus-partitions` resolves each case to exactly one owner from
these defaults or a future per-case `partition` override, rejects unknown
partition names, rejects branch/ref-shaped floating inputs, and rejects holdout
cases that opt into an every-PR cadence.

The partitions are:

```text
conformance:
  exact fixtures and committed surface goldens
  every-PR where cheap enough
  tuned directly by normal development

regression:
  known real repos and PRs
  nightly, release-readiness, or manual
  used to prevent known real-code behavior from drifting

holdout:
  unseen, fresh, or embargoed repos and PRs
  release-readiness or scheduled evaluation only
  result recorded before tuning or detector changes
```

The holdout partition is diagnostic evidence, not a claim of general accuracy.
Holdout failures may create follow-up work, but the first recorded holdout run
must remain visible before the repo adapts to that input.

### Refresh policy

- Pin exact SHAs or checked-in diffs; floating branches are not valid corpus
  entries.
- Rotate part of the holdout set each release cycle when fresh suitable inputs
  are available.
- Retain historic snapshots or reports for trend comparison.
- Do not tune directly against holdout findings before recording the result.
- Promote a holdout case into regression only after the initial result and
  follow-up decision are recorded.
- Keep every partition advisory: no precision, recall, UB-free, Miri-clean,
  site-execution, or memory-safety proof claim is created by partitioning.

## Layer 1: Detector-control corpus

**Already exists** as `policy/detector-contracts.toml` and
`fixtures/calibration.toml`.

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
naming) are enumerated in `fixtures/calibration.toml`. The per-family D1-D5
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

`fixtures/calibration.toml` — fixture-to-expected-cards map with class, operation
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
provides adoption evidence and surfaces friction the corpus layers cannot. This is
not an automated gate. No automatic third-party issue filing. Results are
recorded in `docs/dogfood/` as evidence entries. Output is classified into:
actionable, inherited, noisy, missed, agent-ready, human-only, cost, and
artifact-friction categories.

External pilots should use the public Action or the same artifact bundle shape
that a new adopter would see. Each pilot records:

- setup friction and acquisition method;
- selected comments and intentionally omitted cards;
- one-screen summary usefulness;
- terminology that confused the maintainer or reviewer;
- runtime and artifact-size observations;
- whether the result was agent-ready or human-only.

Human usefulness judgments use this vocabulary:

```text
actionable
correct_but_not_worth_surfacing
inherited
duplicate
human_only
agent_ready
unclear
incorrect
missed_expected_seam
setup_friction
artifact_friction
```

Those judgments are product evidence. They are not calibrated precision or
recall unless a separate labeled evaluation protocol is approved under
SPEC-0026.

---

## Evidence-loss challenge corpus

False negatives are harder to observe than noisy cards. A challenge corpus
records controlled evidence-loss transformations against fixture, real-repo, or
real-PR inputs and checks that the expected ReviewCard movement occurs.

Examples of valid transformations:

- remove a `# Safety` section;
- replace `assert!` with `debug_assert!` where that weakens the guard;
- remove a same-receiver guard;
- change a test call into a bare mention;
- introduce a wrong-receiver guard;
- remove a witness receipt;
- add an unsafe declaration;
- move an unsafe call outside the expected scope.

Expected results are movement-shaped, not proof-shaped:

```text
new gap appears
class changes
comment eligibility changes only when surfacing policy says so
repair route remains correct
artifact bundle stays schema-valid
```

This establishes that known evidence-loss transformations are detected on the
selected inputs. It does not establish global recall, source execution, or
memory-safety proof.

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

```text
spec obligation (SPEC-XXXX clause)
  -> stance-decisions.toml entry (owner-decided stance)
    -> fixtures/<name>/  (detector-control or pure-example layer)
    -> docs/dogfood/corpus.toml target (real-repo layer)
    -> policy/pr-corpus.toml case (real-PR layer)
      -> output surface (cards.json / comment-plan.json / lsp.json / ...)
        -> xtask gate (check-pr / check-fixture-surface-parity / check-surface-determinism / check-real-pr-corpus / check-corpus-partitions)
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
- **Single truth.** Extend `fixtures/calibration.toml` / `corpus.toml` /
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

Post-0.3.8 generalization work continues in review-forward slices:

- GPR-1: partition defaults/checks landed via `partition_default`,
  `partition_by_kind`, and `xtask check-corpus-partitions`; no duplicate corpus
  ledger.
- GPR-2: add the first small holdout set and a release-readiness report format.
- GPR-3: add an evidence-loss challenge harness over a bounded canonical subset.
- GPR-4: run read-only external Action pilots and record human usefulness
  judgments.
- GPR-5: publish a validation closeout that separates conformance, regression,
  holdout, challenge, and pilot evidence.

See `.rails/lanes/corpus/implementation-plan.md` for the full sequence and
evidence grounding.
