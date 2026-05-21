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
