# UNSAFE-REVIEW-SPEC-0010: Policy, baseline, suppressions

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md

## Problem

`unsafe-review` needs a precise, checkable behavior contract for policy, baseline, suppressions.

## Behavior

Use exact counted identity for baselines and suppressions. Default advisory,
then no-new-debt, then calibrated blocking.

The current implementation validates ledger shape and applies advisory
classification for exact card identity matches. Baseline and suppression
entries, when present, must include:

- `card_id`: exact counted `UR-*` review-card identity ending in `-cN`
- `owner`
- `reason`
- `evidence`
- `review_after` for baseline entries
- `review_after` or `expires` for suppression entries

Date fields use `YYYY-MM-DD`. A ledger with `status = "empty"` must not contain
entries.

When a generated card ID exactly matches a baseline entry, the card class is
`baseline_known`, priority is lowered, and the card is excluded from actionable
gap counts. When it exactly matches a suppression entry, the card class is
`suppressed`, priority is lowered, and the card is excluded from actionable gap
counts.

No-new-debt is available only when explicitly requested through
`--policy no-new-debt`. It exits nonzero after rendering output if open
actionable gaps remain. Exact baseline and suppression matches are excluded from
the actionable-gap count.

`policy report` is an advisory-only policy projection. It runs analysis in
advisory mode and reports new gaps, baseline-known cards, suppressed cards,
resolved baseline entries, and expired suppression entries from exact card
identity matching. Current-card entries project ReviewCard identity, location,
operation family, hazards, missing evidence, and witness routes so policy rows
remain reviewable without creating a separate finding truth. Current
`baseline_known` and `suppressed` rows must also include matched ledger
provenance when present. Resolved and expired ledger rows must preserve owner,
reason, evidence, and review/expiry dates when present. It may render JSON or
Markdown. It must not change exit-code policy, create broad suppression
authority, execute witnesses, or claim safety.

Blocking mode remains later policy work. Matching is exact and counted; broad
path, owner, or operation-family suppression is not supported.

## Non-goals

- no soundness claim
- no hidden blocking unless policy mode explicitly enables it
- no duplicate truth outside this spec and linked policy files
- no broad baseline or suppression patterns
- no default no-new-debt or blocking behavior
- no calibrated blocking behavior yet

## Required evidence

- fixture-backed examples for positive and negative cases
- JSON output contract coverage
- xtask policy-ledger schema tests
- analyzer tests for exact baseline and suppression matches
- CLI tests for explicit no-new-debt behavior
- policy documentation when behavior is configurable

## Acceptance examples

- Empty baseline and suppression ledgers pass `check-policy`.
- Non-empty baseline entries require exact counted identity, owner, reason,
  evidence, and `review_after`.
- Non-empty suppression entries require exact counted identity, owner, reason,
  evidence, and either `review_after` or `expires`.
- Uncounted card identities are rejected.
- Exact baseline matches classify cards as `baseline_known`.
- Exact suppression matches classify cards as `suppressed`.
- `--policy no-new-debt` exits nonzero when unbaselined actionable gaps remain.
- `--policy no-new-debt` succeeds when exact baseline matches clear actionable
  gaps.
- `policy report` succeeds in advisory mode, reports unbaselined actionable
  gaps, and does not fail the command for those gaps.
- `policy report` reports exact resolved baseline entries and expired
  suppressions without altering card classification.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
cargo test -p xtask ledger
cargo test -p unsafe-review-core baseline_policy
cargo test -p unsafe-review-core suppression_policy
cargo test -p unsafe-review-core policy_report
cargo test -p unsafe-review-cli no_new_debt
cargo test -p unsafe-review-cli policy_report
cargo test -p unsafe-review --test e2e no_new_debt_policy_fails_only_for_unbaselined_actionable_gaps
cargo test -p unsafe-review --test e2e policy_report
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
