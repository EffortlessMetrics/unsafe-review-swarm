# UNSAFE-REVIEW-SPEC-0007: Test reachability

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md

## Problem

`unsafe-review` needs a precise, checkable behavior contract for test reachability.

## Behavior

Estimate static reach from tests, doctests, fuzz/model/proof harnesses, and
imported receipts without claiming execution proof.

Fixture and golden ReviewCards must keep reach evidence explicitly static:
top-level `reach` and every obligation-level `reach.summary` must use the same
site-owner static-test-mention wording or explicit no-owner-inferred wording,
match `site.owner`, and avoid wording such as site reached, site executed, test
covered, or execution proof unless a separate witness receipt supports that
claim through receipt surfaces.

## Non-goals

- no soundness claim
- no hidden blocking unless policy mode explicitly enables it
- no duplicate truth outside this spec and linked policy files

## Required evidence

- fixture-backed examples for positive and negative cases
- JSON output contract coverage
- human output smoke coverage
- policy documentation when behavior is configurable

## Acceptance examples

- A changed unsafe seam produces one review card with stable identity.
- The card includes missing evidence and a next action.
- A card may say a related test mentions the site owner, but it must not present
  that as proof that the unsafe site executed.
- Top-level reach evidence and obligation-level reach evidence identify the same
  owner and posture.
- If evidence is not knowable statically, the card names the limitation instead of overclaiming.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
