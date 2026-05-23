# UNSAFE-REVIEW-SPEC-0026: Accuracy validation and calibration

Status: proposed
Owner: calibration
Created: 2026-05-21
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked specs:
- UNSAFE-REVIEW-SPEC-0020-source-of-truth-stack
Support-tier impact:
- docs/status/SUPPORT_TIERS.md
Policy impact:
- policy/accuracy-calibration.toml
- policy/public-surfaces.toml

## Problem

unsafe-review has fixture-backed and dogfood-backed evidence, but it does not
yet have a labeled calibration protocol that can justify scoped accuracy claims,
support-tier promotion, or policy readiness.

The repo needs a machine-checkable way to say:

- what was measured,
- on which corpus,
- by whom,
- with what labels,
- with what metrics,
- against which version,
- and what claims are allowed.

## Behavior

Accuracy validation is claim-scoped.

Each calibrated claim names:

- operation family or surface,
- corpus partition,
- label protocol,
- metric definition,
- threshold,
- known limits,
- proof commands,
- support-tier wording.

Fixture-pinned label ledgers may precede human calibration, but they must stay
obligation-level. For a ReviewCard with multiple obligations, the ledger names
the expected obligation key and evidence state instead of treating the card
class as proof that every obligation is discharged.

Fixture golden cards must keep `obligations` and `obligation_evidence` aligned.
Every obligation needs exactly one evidence row with the same description, a
stable key, and coherent `contract`, `discharge`, `reach`, and `witness`
present/state/summary fields. This prevents top-level card classes or comments
from standing in for per-obligation evidence.

Fixture golden cards must keep top-level evidence summaries aligned with
obligation-level evidence. Top-level `contract` and `witness` summaries mirror
the matching obligation-level summary for that axis. Top-level `discharge` is
allowed to aggregate per-obligation guard state, but only as the checked
all-missing, mixed, all-present, or caller-contract declaration posture.

Fixture golden cards must keep the top-level `missing` summaries aligned with
the per-obligation evidence rows: at least one summary is required when any
contract, discharge, reach, or witness evidence is missing, and no missing
summary is allowed when all obligation evidence is present. The summary remains
reviewer-facing; it is not a replacement for obligation-level evidence.
Stable missing-summary categories must agree with the corresponding evidence
axis: missing contract evidence requires contract-missing wording, missing
discharge evidence requires guard-missing wording, missing witness evidence
requires witness-missing wording, and those summaries must not remain after the
axis is present. Reach-missing summaries are allowed when static reach is
missing, but must not appear when reach evidence is present.

Fixture golden cards must keep reach evidence aligned with the static
test-reachability boundary. The top-level `reach` summary and every
obligation-level reach summary must match, identify the same `site.owner` or an
explicit no-owner-inferred posture, and describe only static test mentions, the
absence of a static test mention, or inability to infer an owner. They must not
claim site execution, site reach, coverage, or execution proof without a witness
receipt surface.

Fixture golden cards must keep witness routes and verify commands aligned. Each
card needs at least one advisory route, route kinds must belong to the operation
family registry row, command-bearing routes must name the matching witness tool
and be mirrored in `verify_commands`, unbacked verify commands are rejected, and
routes must not be marked required by default.

Fixture golden cards must preserve counted ReviewCard identity components:
package/fixture, file, owner, site kind, operation family, operation path or
callee token, normalized snippet hash, hazard, and count suffix. This keeps
calibration, baselines, suppressions, and receipts tied to the same unsafe seam
instead of to a broad operation-family bucket.

When a claim is about public contract evidence, the ledger must pin
`contract.state` from the matching ReviewCard obligation evidence. A local
`SAFETY:` comment may document a nearby unsafe block, but it must not satisfy a
public unsafe API `# Safety` documentation claim unless the ReviewCard contract
evidence records it as public API documentation.

When a claim is about witness routing, the ledger must pin route kinds from the
matching ReviewCard `witness_routes`. A route-quality label proves only that the
static card recommends the expected next witness route; it does not prove the
witness was run or passed.

When a claim is about no-card artifact honesty, the ledger pins
`expected_cards = 0` for the named fixture and must not include per-card
operation, hazard, obligation, evidence-state, or witness-route expectations.
The claim proves only that the listed fixtures emit zero ReviewCards. It does
not justify "all clear", safety, UB-free, Miri-clean, or site-execution wording.

## Non-goals

- no global precision/recall claim,
- no memory-safety proof,
- no UB-free claim,
- no Miri-clean claim,
- no default blocking policy,
- no automatic support-tier promotion,
- no training on held-out samples after they are designated holdout.

## Required evidence

- fixture calibration manifest passes,
- dogfood corpus manifest passes,
- labeled sample ledger validates,
- `docs/accuracy/CALIBRATION_REPORT.md` renders and is checked for stale
  counts and no-overclaim boundary text by `check-calibration`,
- claim entries pass promotion guardrails for status-specific evidence,
  known support-tier capability names, known dogfood target IDs, checked labeled
  report files, and public wording that stays inside the supported claim,
- fixture golden obligations and obligation evidence are one-to-one,
  description-aligned, and carry coherent per-axis evidence states,
- fixture golden top-level contract, discharge, and witness summaries are
  coherent projections of obligation-level evidence, not independent evidence,
- fixture golden obligation evidence keys belong to the operation family
  registry row,
- fixture golden missing summaries are non-empty exactly when at least one
  obligation evidence axis is missing and stable contract/discharge/witness
  missing categories match their evidence axes,
- fixture golden reach evidence keeps top-level and obligation-level summaries
  aligned with `site.owner` and static-test-mention wording, without execution
  proof overclaims,
- fixture golden next actions are non-empty reviewer actions, avoid overclaim
  wording, and name the matching operation family when they refer to a safety
  obligation,
- fixture golden witness routes are advisory, non-empty, command-kind coherent,
  and command-aligned with verify commands, and route kinds belong to the
  operation family registry row,
- allowed public claim wording names the claim level, and forbidden claim lists
  include shared global precision, global recall, and memory-safety proof
  overclaim boundaries,
- every claim fixture is backed by a label sample, and every label sample stays
  inside the owning claim's fixture list,
- label sample counts do not double-count the same fixture, obligation, and
  evidence expectation within one claim,
- fixture golden ReviewCard IDs preserve fixture/package, file, owner, site
  kind, operation family, operation path/callee, snippet hash, hazard, and
  counted identity suffix,
- fixture golden operation families and hazards use known domain vocabulary,
  hazards belong to the operation family registry row, and hazard lists do not
  contain duplicates,
- fixture golden site metadata uses known site kinds, known visibility values,
  positive source coordinates, relative Rust source paths, coherent public API
  flags, and one operation expression shared by `operation` and `site.snippet`,
- fixture golden class, priority, and confidence fields use known ReviewCard
  values and fixture-pinned classification signals,
- support-tier claim map matches measured evidence,
- no-overclaim checks pass.

## Acceptance examples

- A false-positive-control fixture cannot be omitted from calibration.
- A labeled sample cannot lack an adjudicated expected outcome.
- A support-tier promotion cannot reference a metric report that does not exist.
- A report cannot claim calibrated precision/recall without labeled denominator data.
- A calibrated claim cannot name a support tier that is absent from
  `docs/status/SUPPORT_TIERS.md`.
- A label ledger cannot add fixture evidence outside the owning claim's
  `fixtures` list.
- A claim cannot list a fixture that has no matching label sample.
- A claim cannot count the same fixture/obligation/evidence expectation twice
  through one or more label ledgers.
- A fixture-pinned claim cannot carry labeled reports or dogfood targets.
- A dogfood-measured claim cannot reference an unknown dogfood target.
- A fixture card cannot introduce an unknown site kind, invalid source
  coordinate, private public-API flag, or operation/snippet mismatch.
- A fixture card cannot introduce an unknown operation family, unknown hazard,
  hazard outside the operation family registry row, or duplicate hazard.
- A fixture card cannot introduce an obligation evidence key outside the
  operation family registry row.
- A fixture card cannot drift top-level contract, discharge, or witness
  summaries away from the matching obligation-level evidence posture.
- A fixture card cannot omit required contract, guard, or witness missing
  summaries, or keep stale missing summaries after the corresponding evidence
  axis is present.
- A fixture card cannot drift obligation-level reach away from top-level reach,
  name a different owner, or claim a test reached or executed the unsafe site.
- A fixture card cannot introduce a witness route kind outside the operation
  family registry row.
- A fixture card cannot attach a cargo-careful command to a Miri route or attach
  a concrete command to a manual-only route by default.
- A fixture card cannot introduce an unknown class, priority, or confidence, or
  pair a supported class with a stale priority/confidence signal.
- An allowed public claim cannot contain global precision/recall, policy-ready,
  UB-free, Miri-clean, or memory-safety proof wording.
- A fixture card next action cannot say "all clear" or name the wrong operation
  family for safety-obligation repair guidance.
- An allowed public claim cannot omit its claim level, such as
  `Fixture-pinned`.
- A forbidden claim list cannot omit the shared global precision, global recall,
  or memory-safety proof boundaries.
