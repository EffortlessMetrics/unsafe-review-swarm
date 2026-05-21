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

## Projection contract

PR CI has two separate responsibilities:

- artifact contract checks
- advisory unsafe-review findings

The gate may fail when the tool cannot run, an artifact is missing or malformed,
a schema/trust-boundary contract is violated, or an output overclaims the
evidence. A ReviewCard finding is advisory by default and must not become a
blocking PR decision unless an explicit policy mode says so.

`comment-plan.json` is the only inline-comment surface for v0.x. It is
plan-only, capped at three candidate comments, restricted to changed lines, and
limited to high-signal actionable cards. It must not include `static_unknown`,
baseline-known, or suppressed cards. No workflow posts the plan by default; a
future trusted poster must consume this artifact rather than regenerating its
own analyzer truth.

PR CI uses two explicit gates:

- **Artifact integrity gate (hard-fail):** prove `unsafe-review first-pr` ran,
  emitted required artifacts, and kept trust-boundary/overclaim rules.
- **Unsafe evidence policy gate (advisory by default):** report findings,
  baseline/suppression state, and witness posture without failing by default.

The default lane for 0.2.0 is integrity-hard + policy-advisory.

For first-pr runs, the artifact bundle contract is:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/lsp.json
target/unsafe-review/policy-report.json
target/unsafe-review/policy-report.md
```

`lsp.json` may be empty-by-content when there are no applicable diagnostics, but
the saved projection contract remains part of the first-pr bundle.

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
- `unsafe-review first-pr --base origin/main` writes the first-pr advisory
  bundle in `target/unsafe-review/`.
- The advisory workflow uploads the first-pr bundle artifacts without running
  Miri, posting comments, or enabling blocking policy.
- The advisory workflow runs `cargo xtask check-first-pr-artifacts
  target/unsafe-review` before
  upload so malformed artifacts fail the advisory job instead of being published
  as trusted dogfood evidence.
- Empty output states no actionable cards and does not imply the repository is
  safe or UB-free.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
cargo xtask check-first-pr-artifacts target/unsafe-review
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
