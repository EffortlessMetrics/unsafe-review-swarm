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

When a claim is about public contract evidence, the ledger must pin
`contract.state` from the matching ReviewCard obligation evidence. A local
`SAFETY:` comment may document a nearby unsafe block, but it must not satisfy a
public unsafe API `# Safety` documentation claim unless the ReviewCard contract
evidence records it as public API documentation.

When a claim is about witness routing, the ledger must pin route kinds from the
matching ReviewCard `witness_routes`. A route-quality label proves only that the
static card recommends the expected next witness route; it does not prove the
witness was run or passed.

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
- calibration report renders,
- support-tier claim map matches measured evidence,
- no-overclaim checks pass.

## Acceptance examples

- A false-positive-control fixture cannot be omitted from calibration.
- A labeled sample cannot lack an adjudicated expected outcome.
- A support-tier promotion cannot reference a metric report that does not exist.
- A report cannot claim calibrated precision/recall without labeled denominator data.
