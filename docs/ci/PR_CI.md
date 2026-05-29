# PR and CI model

This guide is the operator-facing companion to
[UNSAFE-REVIEW-SPEC-0024: CI design](../specs/UNSAFE-REVIEW-SPEC-0024-ci-design.md).
`UNSAFE-REVIEW-SPEC-0011` remains the artifact contract for PR output. This
guide explains how CI lanes use that contract.

The core line:

```text
Malformed or dishonest unsafe-review artifacts fail CI.
Unsafe-review findings do not fail CI by default.
```

CI lanes have separate jobs and authority:

- Default workspace CI protects build, lint, tests, docs, and repo policy.
- Policy-contracts validates source-of-truth ledgers and goal/spec rails.
- First-pr advisory CI renders and verifies the public PR packet.
- Source-divergence reports source/swarm drift before routine swarm work.
- Coverage is optional execution-surface telemetry, not proof.
- Release readiness is explicit package/install smoke proof, not every-PR cost.
- Trusted comment posting is future split-token infrastructure, not default CI.

## Default workspace gate

Default PR CI runs the cheap repository policy gate on the pinned Rust
toolchain. The full workspace proof set remains:

```text
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo run --locked -p xtask -- check-pr
```

This lane protects repository correctness: formatting, build, lint, tests,
rustdoc, and repo policy checks. The live swarm workflow may route a cheaper
Rust Small lane through `cargo run --locked -p xtask -- check-pr`; broader
workspace checks remain local, release, or future full-lane proof until a live
workflow explicitly promotes them.

It must not run:

```text
Miri
cargo-careful
sanitizers
Loom
Kani
mutation testing
comment posting
source edits
publish
```

The CI workflow keeps repository permissions read-only, avoids persisted
checkout credentials, cancels superseded pull request runs, supports manual
dispatch for ad hoc verification, and bounds the Rust job with a timeout.
Dependabot opens weekly Cargo and GitHub Actions update PRs as maintenance
signals; those PRs still pass through the same advisory CI and review process.
The `dtolnay/rust-toolchain` action ref is intentionally pinned to the repo
toolchain version and is not Dependabot-managed.

Default analyzer and artifact lanes must not request write tokens. A workflow
that can post comments, mutate branches, publish crates, or write releases must
be specified as a separate trusted lane before it is introduced.

## Policy contracts lane

The policy contracts lane validates the source-of-truth rails without running
unsafe-review analysis. The full lane contract is:

```text
cargo run --locked -p xtask -- check-doc-artifacts
cargo run --locked -p xtask -- check-docs-automation
cargo run --locked -p xtask -- check-goals
cargo run --locked -p xtask -- check-package-boundary
cargo run --locked -p xtask -- check-ci-lanes
cargo run --locked -p xtask -- check-policy
```

This lane may fail on malformed or drifting source-of-truth ledgers. It must not
run witnesses, post comments, publish, edit source, or turn unsafe-review
findings into a default blocking policy.

During the swarm CI budget window, policy-contracts runs on pull requests only
when source-of-truth rails change:

```text
policy/**
.unsafe-review-spec/**
docs/specs/**
docs/status/**
.github/workflows/**
xtask/**
```

The default workspace gate still runs `check-pr`, so policy checks remain
covered on ordinary PRs without paying for a duplicate workflow every time.

## Editor extension packaging lane

The editor extension packaging lane is path-scoped to the saved-LSP viewer and
its workflow:

```text
editors/vscode/**
.github/workflows/editor-extension.yml
```

Policy or ledger-only changes should not package the extension. The lane still
has read-only permissions, uploads only the VSIX artifact, and does not publish,
run witnesses, post comments, or edit source.

## Advisory first-pr packet lane

The first-pr lane produces the user-facing unsafe-review packet:

```text
unsafe-review first-pr --base origin/<base> --out-dir target/unsafe-review
cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review
```

The bundle contains:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/github-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/lsp.json
target/unsafe-review/repair-queue.json
```

The PR summary artifact is the reviewer front panel. It projects existing
ReviewCards only: counts, top card, card table, witness plan, and the trust
boundary. It must not add PR-specific analyzer truth and must not imply a
blocking policy.

The GitHub summary artifact is the bounded job-summary fragment. It projects
the same ReviewCards, stays capped for CI display, and points reviewers to the
full advisory bundle instead of duplicating the full PR summary.

The SARIF artifact projects the same ReviewCards into code-scanning shape. It
is still advisory static review evidence; uploading SARIF must not be treated as
proof that the changed code is memory-safe.

The comment-plan artifact is a plan of candidate high-signal inline comments.
It is not posted by default.
Fixture-backed selected and not-selected examples are in
[COMMENT_PLAN_EXAMPLES.md](COMMENT_PLAN_EXAMPLES.md).

The saved `lsp.json` artifact is a read-only projection for diagnostics,
hovers, and command-only actions. It must not include `WorkspaceEdit`, source
edits, witness execution, comment posting, or policy approval actions.

The `repair-queue.json` artifact groups ReviewCards into copy-only agent handoff
buckets such as guard, contract, test, witness receipt, human review, and
do-not-auto-repair. It points back to `unsafe-review context <card-id> --json`
and must not imply that unsafe-review ran an agent or applied a repair.

The first-pr gate fails on infrastructure and artifact contract failures:

- tool invocation failed,
- required artifact missing,
- machine-readable artifact malformed,
- card IDs inconsistent across projections,
- trust boundary missing,
- output contains positive safety/proof wording,
- comment plan violates its artifact contract,
- saved LSP violates its read-only projection contract,
- witness-plan route limits are missing.

It does not fail because advisory findings exist:

- new cards,
- guard-missing cards,
- contract-missing cards,
- missing witness receipts,
- advisory policy-report gaps.

The comment-plan contract is intentionally narrow:

- at most three planned comments,
- changed lines only,
- high-confidence actionable cards only,
- no duplicate card IDs or duplicate inline anchors,
- comment bodies stay within the hard 220-word limit,
- no `static_unknown`, baseline-known, or suppressed cards,
- no posting by default.

A future trusted poster must consume `comment-plan.json` and keep the same
ReviewCard identity, next action, actionability, relevance, witness route,
verify-command, and trust-boundary fields.
It must not rerun analysis and create a second comment truth.
Card-present/no-comment cases must be represented through `not_selected`
entries in `comment-plan.json`, so reviewers can see why a card stayed out of
the inline comment budget, including its operation and next-action context,
without opening a second analyzer truth.
The trusted poster architecture is specified in
[TRUSTED_COMMENT_POSTER.md](TRUSTED_COMMENT_POSTER.md); it remains future
infrastructure and is not a default workflow.

## Witness posture

The default workflows do not run Miri, cargo-careful, sanitizers, Loom, Kani,
Crux, fuzzing, or other witness tools.

Witness tools are routed, not run everywhere. They belong in targeted PR,
nightly, release, or explicit manual/receipt lanes unless repo policy says
otherwise. CI may generate `witness-plan.md`, but it must not fabricate receipts
or claim a witness ran unless a matching receipt was imported.

## Coverage and release lanes

Coverage belongs in a separate advisory lane. It is Rust test execution-surface
telemetry, not unsafe correctness evidence, memory-safety proof, UB-free status,
or witness adequacy.

Release readiness belongs to release lanes, not every PR. Expected release
proof includes workspace checks, `check-pr`, `check-calibration`,
`check-dogfood`, package lists, publish dry-runs, and install/first-pr/support
smokes. Release readiness must not publish by itself unless a separate trusted
release workflow is specified.

## Source/swarm sync

Routine implementation belongs in `unsafe-review-swarm`; source publishes
curated promotions and release/public-surface work.

Before routine swarm work, run:

```text
cargo run --locked -p xtask -- source-divergence
```

If source has unmirrored implementation commits, pause routine feature work and
open a swarm sync or acknowledgement PR before continuing.

## Copy-paste first-pr workflow

For a drop-in advisory PR lane, copy
`.github/examples/unsafe-review-first-pr.yml`. It runs one `first-pr` command,
verifies the full artifact bundle contract, uploads all first-run artifacts, and
writes a GitHub job summary.

Default behavior of the example workflow:

- runs `unsafe-review first-pr --base origin/<base>`;
- verifies with `cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review`;
- uploads `cards.json`, `pr-summary.md`, `github-summary.md`, `cards.sarif`,
`comment-plan.json`, `witness-plan.md`, `lsp.json`, and `repair-queue.json`;
- does not post comments;
- does not run witnesses;
- does not block on findings, only on artifact/tooling contract failures.
