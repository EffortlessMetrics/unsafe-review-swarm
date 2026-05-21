# UNSAFE-REVIEW-SPEC-0011: PR and CI output

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md

## Problem

`unsafe-review` needs a precise, checkable behavior contract for pr and ci output.

## Behavior

PR output must be sparse: summary first, at most a few high-confidence inline comments, durable JSON/Markdown/SARIF artifacts.
The first supported PR projection is a local Markdown summary artifact rendered
from existing `ReviewCard`s. It includes counts, a top card, a card table, a
witness plan, and the trust boundary. The top card and table include the
ReviewCard operation expression and operation family so reviewers can identify
the exact unsafe operation and group findings without reclassifying them. It
does not post comments, run witness tools, or change policy mode.
SARIF output is also a projection from existing `ReviewCard`s. SARIF results
carry card identity, operation expression, operation family, hazards, missing
evidence, witness route recommendations, structured route details, verify
commands, and the same trust boundary in result properties.
The advisory GitHub workflow uploads the JSON, Markdown summary, SARIF, and
comment-plan artifacts. It does not run witness tools, post inline comments, or
enable blocking policy.
Inline comment planning is artifact-only. The plan contains candidate comments
for actionable high-priority or high-confidence cards, but no workflow posts
those comments by default. Each planned comment carries the same ReviewCard
operation expression, witness route details, and verify commands used by JSON,
SARIF, and LSP projections so review bots and maintainers do not need to parse
comment prose or reclassify findings.

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
- If evidence is not knowable statically, the card names the limitation instead of overclaiming.
- `unsafe-review check --format pr-summary --out target/unsafe-review/pr-summary.md`
  writes a GitHub-ready Markdown artifact.
- `unsafe-review check --format sarif --out target/unsafe-review/cards.sarif`
  writes parseable SARIF 2.1.0.
- `unsafe-review check --format comment-plan --out target/unsafe-review/comment-plan.json`
  writes candidate inline comments without posting them.
- The advisory workflow uploads `cards.json`, `pr-summary.md`, `cards.sarif`,
  and `comment-plan.json` as artifacts without running Miri, posting comments,
  or enabling blocking policy.
- The advisory workflow runs `cargo xtask check-advisory-artifacts` before
  upload so malformed artifacts fail the advisory job instead of being published
  as trusted dogfood evidence.
- Empty output states no actionable cards and does not imply the repository is
  safe or UB-free.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
cargo xtask check-advisory-artifacts target/unsafe-review
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
