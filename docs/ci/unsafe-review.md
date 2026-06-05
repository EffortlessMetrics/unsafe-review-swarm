# unsafe-review CI lane

`unsafe-review` is advisory unsafe-contract review. It checks whether changed
unsafe seams have reviewable evidence: a safety contract, local guard, test
reach, and witness route.

It does not prove memory safety, UB-free status, Miri-clean status, or site
execution. A matching witness receipt records external evidence for that
specific route only.

## Tool split

| Tool | Question |
| --- | --- |
| Source exception ledger / future `cargo-allow` wrapper | Is this unsafe or source exception allowed and owned? |
| `unsafe-review` | Is this unsafe seam reviewable: contract, guard, test reach, witness route? |
| Miri / sanitizers / witnesses | Did a concrete execution expose UB or memory misuse? |

These are complementary planes. An owned `unsafe` exception is not automatically
reviewable, and a ReviewCard is not a Miri receipt.

## Repo-facing surface

The durable PR surface should remain `xtask` and ReviewCard-derived artifacts,
for example:

```bash
cargo run --locked -p xtask -- unsafe-review-pr
```

Current and future wrappers must preserve the advisory product boundary:

```text
unsafe-review finds unsafe Rust changes missing a safety contract, guard, test,
or witness.
```

## Expected artifacts

The unsafe-review lane may produce:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/github-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/lsp.json
target/unsafe-review/receipt-audit.md
```

All projections must come from ReviewCards. CLI output, JSON, Markdown summaries,
SARIF, saved LSP diagnostics, hovers, code actions, agent packets, badges,
baselines, suppressions, and witness receipts must not create separate analyzer
truths outside ReviewCard.

## CI posture

Default CI may fail malformed or dishonest artifacts. It must not fail merely
because advisory unsafe-review findings exist. Requiring evidence or waiver for
changed unsafe seams is a later policy promotion and must be named in the policy
ledger before it becomes blocking.
