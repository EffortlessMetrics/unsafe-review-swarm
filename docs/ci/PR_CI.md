# PR and CI model

Default swarm PR runs cheap static review on the pinned Rust toolchain:

```text
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- check-pr
unsafe-review check --base origin/main --format json
unsafe-review check --base origin/main \
  --format pr-summary \
  --out target/unsafe-review/pr-summary.md
unsafe-review check --base origin/main \
  --format sarif \
  --out target/unsafe-review/cards.sarif
unsafe-review check --base origin/main \
  --format comment-plan \
  --out target/unsafe-review/comment-plan.json
```

The CI workflow keeps repository permissions read-only, avoids persisted checkout
credentials, cancels superseded synchronize runs, supports manual dispatch for
ad hoc verification, and bounds the Rust job with a timeout. In
`unsafe-review-swarm`, the normalized result check is documented in
[`SWARM_CI.md`](SWARM_CI.md).
Dependabot opens weekly Cargo and GitHub Actions update PRs as maintenance
signals; those PRs still pass through the same advisory CI and review process.
The `dtolnay/rust-toolchain` action ref is intentionally pinned to the repo
toolchain version and is not Dependabot-managed.

The PR summary artifact is Markdown for GitHub job summaries or uploaded
artifacts. It projects existing review cards only: counts, top card, card table,
witness plan, and the trust boundary. It must not add PR-specific analyzer truth
and must not imply a blocking policy.

The SARIF artifact projects the same review cards into code-scanning shape. It
is still advisory static review evidence; uploading SARIF must not be treated as
proof that the changed code is memory-safe.

The routed CI workflow writes and uploads:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
```

Before upload, the workflow runs:

```text
cargo run --locked -p xtask -- check-advisory-artifacts target/unsafe-review
```

The comment plan is an artifact of at most three candidate high-signal inline
comments. It is not posted by the workflow.

The workflow does not run Miri, sanitizers, Loom, Kani, or other witness tools.
It does not post comments and does not enable blocking policy.

After downloading or rendering an advisory artifact set, verify the artifact
contract with:

```text
cargo xtask check-advisory-artifacts target/unsafe-review
```

This checks that `cards.json`, `pr-summary.md`, `cards.sarif`, and
`comment-plan.json` exist, machine-readable artifacts parse, the policy remains
advisory, the comment plan remains plan-only, projected card IDs match
`cards.json`, result counts stay consistent, `cards.json` cards keep required
ReviewCard fields, SARIF results keep their ReviewCard metadata and locations,
the PR summary keeps its top-card, card-table, and witness-plan sections, and
the trust boundary is present.

Witness tools are routed, not run everywhere. Miri, sanitizers, Loom, and Kani
belong in targeted PR, nightly, or release lanes unless repo policy says
otherwise.
