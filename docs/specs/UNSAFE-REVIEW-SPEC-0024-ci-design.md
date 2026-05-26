# UNSAFE-REVIEW-SPEC-0024: CI design

Status: accepted
Owner: repo-infra / ci
Created: 2026-05-21

Linked specs:
- [UNSAFE-REVIEW-SPEC-0011: PR and CI output](UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md)
- [UNSAFE-REVIEW-SPEC-0012: LSP and editor projection](UNSAFE-REVIEW-SPEC-0012-lsp-editor-projection.md)
- [UNSAFE-REVIEW-SPEC-0019: First-run cockpit](UNSAFE-REVIEW-SPEC-0019-first-run-cockpit.md)
- [UNSAFE-REVIEW-SPEC-0020: Source-of-truth stack](UNSAFE-REVIEW-SPEC-0020-source-of-truth-stack.md)
- [UNSAFE-REVIEW-SPEC-0022: PR commenting experience](UNSAFE-REVIEW-SPEC-0022-pr-commenting-experience.md)
- [UNSAFE-REVIEW-SPEC-0023: First-hour experience](UNSAFE-REVIEW-SPEC-0023-first-hour-experience.md)

Linked docs:
- [docs/ci/PR_CI.md](../ci/PR_CI.md)
- [docs/ci/TRUSTED_COMMENT_POSTER.md](../ci/TRUSTED_COMMENT_POSTER.md)
- [.github/workflows/ci.yml](../../.github/workflows/ci.yml)
- [.github/workflows/unsafe-review.yml](../../.github/workflows/unsafe-review.yml)
- [.github/examples/unsafe-review-first-pr.yml](../../.github/examples/unsafe-review-first-pr.yml)
- [docs/contributing/SWARM_TO_MAIN.md](../contributing/SWARM_TO_MAIN.md)
- [docs/BADGE_POLICY.md](../BADGE_POLICY.md)

Support-tier impact:
- [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:
- [policy/ci-lane-whitelist.toml](../../policy/ci-lane-whitelist.toml)
- [policy/doc-artifacts.toml](../../policy/doc-artifacts.toml)
- [policy/package-boundary.toml](../../policy/package-boundary.toml)
- [policy/workflow-allowlist.toml](../../policy/workflow-allowlist.toml)

## 1. Purpose

`unsafe-review` CI must protect two things at the same time:

```text
the Rust workspace
the advisory unsafe-review artifact contract
```

CI must make the repo safe to maintain and useful to users without overstating
what the tool proves.

The default CI design is:

```text
cheap deterministic workspace checks
+ advisory first-pr packet verification
+ no witness execution by default
+ no automatic comments by default
+ no blocking on unsafe-review findings by default
```

CI proves that the tool and artifacts are well formed. It does not prove the
reviewed Rust code is safe.

`UNSAFE-REVIEW-SPEC-0011` remains the owner of PR and CI output artifacts:
first-pr bundle shape, artifact verification, advisory findings, comment-plan
behavior, and the distinction between artifact failures and unsafe-review
findings. This spec owns the broader CI lane design.

This spec owns:

```text
default workspace CI
advisory PR review lane
artifact integrity gate
source/swarm sync guard
coverage / Codecov lane
release readiness lane
future trusted comment poster lane
security and token posture
runner/cost posture
```

## 2. Core doctrine

CI has four different jobs.

```text
workspace correctness
  Does the repo build, lint, test, and document?

artifact integrity
  Did unsafe-review produce parseable, internally consistent, honest artifacts?

advisory evidence
  What unsafe-review cards, witness routes, and posture changes should reviewers inspect?

release readiness
  Can the published crates be packaged, installed, and smoke-tested?
```

The first two may fail default CI.

The third is advisory by default.

The fourth belongs to release lanes, not every PR.

The core line:

```text
Malformed or dishonest unsafe-review artifacts fail CI.
Unsafe-review findings do not fail CI by default.
```

## 3. Default CI contract

The default workflow runs on:

```text
pull_request
push to main/master
workflow_dispatch
```

It uses read-only repository permissions, cancels superseded pull request runs,
disables persisted checkout credentials, and bounds Rust jobs with timeouts.

The full workspace proof set is:

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo run --locked -p xtask -- check-pr
```

That is the baseline.

The live swarm workflow may route a cheaper Rust Small lane through
`cargo run --locked -p xtask -- check-pr` while broader workspace checks remain
local, release, or future full-lane proof. That routing must stay explicit and
must not smuggle witnesses, publishing, comment posting, or source edits into
the default gate.

## 4. CI lane taxonomy

Every live or planned CI lane must have a named purpose. High-cost or
write-token lanes must not be folded into the default workspace gate by
convenience.

### 4.1 `ci.yml` - default workspace gate

Purpose:

```text
protect the Rust workspace and repo policy checks
```

Runs:

```text
fmt
check
clippy
tests
docs
xtask check-pr
```

May fail on:

```text
formatting drift
build failure
lint failure
test failure
rustdoc warning
repo policy failure
```

Must not run:

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

Default permissions:

```yaml
permissions:
  contents: read
```

### 4.2 `policy-contracts.yml` - source-of-truth gate

Purpose:

```text
protect spec, policy, package-boundary, docs-automation, goal, and CI-lane ledgers
```

Runs:

```text
check-doc-artifacts
check-docs-automation
check-goals
check-package-boundary
check-ci-lanes
check-policy
```

During the swarm CI budget window, pull-request runs are path-scoped to
source-of-truth rails:

```text
policy/**
.unsafe-review-spec/**
docs/specs/**
docs/status/**
.github/workflows/**
xtask/**
```

The default `check-pr` lane still covers the policy bundle on ordinary PRs, so
policy-contracts is a focused duplicate-proof lane rather than a second default
PR cost center.

May fail on malformed or drifting source-of-truth rails. It must not run
first-pr analysis, witnesses, coverage, publishing, comment posting, or source
mutation.

### 4.3 `unsafe-review-first-pr.yml` - advisory PR packet lane

Purpose:

```text
produce and verify the user-facing unsafe-review PR packet
```

Command path:

```bash
unsafe-review first-pr \
  --base origin/<base> \
  --out-dir target/unsafe-review

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review
```

Artifacts:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/lsp.json
```

The drop-in example workflow follows this shape. The live swarm advisory
workflow may be tightened toward this lane, but it must preserve read-only
permissions, no comment posting, no witness execution, and no source edits.
It should build `unsafe-review`, run one `first-pr` command, verify the bundle,
write a GitHub summary, and upload the first-pr artifacts.

May fail on:

```text
unsafe-review could not run
required artifact missing
artifact malformed
card IDs inconsistent across artifacts
trust boundary missing
positive safety/proof wording
comment-plan contract violation
saved LSP contract violation
witness-plan route-limit violation
```

Must not fail on:

```text
cards exist
guard_missing exists
contract_missing exists
witness missing
policy report has advisory new gaps
```

Must not do:

```text
post comments
run witnesses
edit source
enable blocking policy
claim safety
```

### 4.4 `coverage.yml` - advisory coverage / Codecov lane

Purpose:

```text
publish Rust test execution-surface telemetry
```

Coverage is useful public signal, but it is not unsafe correctness evidence.

Recommended command:

```bash
cargo llvm-cov --workspace --all-targets --locked \
  --lcov \
  --output-path target/llvm-cov/lcov.info
```

Initial posture:

```text
advisory
no coverage threshold gate
no default PR run during the swarm CI budget window
no default PR failure on Codecov upload failure
no release readiness claim
no unsafe correctness claim
no Miri-clean claim
no README badge until the first successful upload
```

The live swarm coverage lane is push/manual only while CI budget mode is active.
Coverage remains telemetry; it is not part of the ordinary PR gate.

### 4.5 `release-readiness.yml` - manual release proof lane

Purpose:

```text
prove package and install readiness before publication
```

Trigger:

```text
workflow_dispatch
release-prep branch
tag candidate, if used later
```

Commands:

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- check-pr
cargo run --locked -p xtask -- check-calibration
cargo run --locked -p xtask -- check-dogfood

cargo package -p unsafe-review-core --list
cargo package -p unsafe-review-cli --list
cargo package -p unsafe-review --list

cargo publish -p unsafe-review-core --dry-run
cargo publish -p unsafe-review-cli --dry-run
cargo publish -p unsafe-review --dry-run
```

Release readiness may prove package list correctness, publish dry-run
correctness, install smoke, first-pr smoke, support smoke, and docs.rs readiness
after publication.

It must not publish by itself unless a separate trusted release workflow is
specified and owner approved.

### 4.6 `source-divergence.yml` or local-only sync guard

Purpose:

```text
prevent unsafe-review-swarm from drifting behind unsafe-review
```

Command:

```bash
cargo run --locked -p xtask -- source-divergence
```

Alias, when present:

```bash
cargo run --locked -p xtask -- check-source-sync
```

Default behavior:

```text
advisory report
not a hard CI failure at first
```

May become a hard check for routine swarm work once the routing policy
stabilizes.

The source/swarm model must remain:

```text
unsafe-review-swarm develops
unsafe-review publishes
```

### 4.7 Future `comment-poster.yml` - trusted poster lane

Purpose:

```text
post or update PR comments from verified comment-plan.json
```

This is not part of 0.2.x default behavior.

Required architecture:

```text
pull_request workflow:
  run analyzer with read-only permissions
  verify artifacts
  upload comment-plan.json

trusted workflow:
  download artifacts
  verify comment-plan.json again
  post/update comments
```

The trusted poster must consume `comment-plan.json`.

It must not:

```text
rerun analysis
run witnesses
edit source
insert suppressions
post more than the plan
post from malformed artifacts
```

Security reason: write-token workflows must not combine untrusted
PR-controlled code execution with comment-writing authority.

The future trusted-poster architecture is specified in
[docs/ci/TRUSTED_COMMENT_POSTER.md](../ci/TRUSTED_COMMENT_POSTER.md). It is not
part of the default workflow set and must remain deferred until separately
implemented and reviewed.

## 5. CI permissions policy

Default workspace, policy-contracts, first-pr, source-divergence, and coverage
lanes use:

```yaml
permissions:
  contents: read
```

Add only if needed:

```yaml
security-events: write
```

for SARIF upload.

Do not grant these to default analyzer jobs:

```yaml
contents: write
pull-requests: write
issues: write
actions: write
id-token: write
```

Manual `cargo publish` remains local/operator-driven unless a dedicated trusted
release workflow is specified.

Future trusted comment posting may use:

```yaml
permissions:
  contents: read
  pull-requests: write
```

only in a workflow that does not run PR-controlled code before posting.

## 6. Checkout and token posture

All workflows must prefer:

```yaml
- uses: actions/checkout@v6
  with:
    persist-credentials: false
```

Exception:

```text
a deliberately trusted commit-back workflow
```

Such workflows must be isolated and separately specified. No job should keep
credentials around merely because it is convenient.

All live workflow actions must be listed in
`policy/workflow-allowlist.toml` with explicit `@` refs. Branch-floating refs
such as `@main`, `@master`, or `@HEAD` are rejected; use a reviewed version tag
or immutable SHA.

## 7. Toolchain, runner, and cost posture

The repo toolchain is Rust 1.95.0.

CI should install the pinned toolchain and Rust components:

```yaml
- uses: dtolnay/rust-toolchain@1.95.0
  with:
    components: rustfmt, clippy
```

Rust version drift must be caught by repo checks and docs.

If the repo later introduces an MSRV matrix, it should be an explicit
compatibility lane, not a surprise expansion of default PR cost.

Default PR CI must stay cheap enough to run on every pull request without
turning advisory unsafe-review into a heavy witness system.

The default posture is:

```text
ubuntu-latest runners
bounded job timeouts
no default matrix
no default nightly-only tools
no default witness execution
no publish or release side effects
```

Swarm may carry experimental, scheduled, or workflow-dispatch lanes while they
are being proven, but a lane must be listed in
`policy/ci-lane-whitelist.toml` with its cost estimate and trigger policy
before it becomes a live workflow.

## 8. Artifact integrity checks

The first-pr artifact checker is a CI gate.

Command:

```bash
cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review
```

It must validate:

```text
cards.json exists and parses
pr-summary.md exists
cards.sarif exists and parses
comment-plan.json exists and parses
witness-plan.md exists
lsp.json exists and parses

card IDs align across artifacts
result counts stay internally consistent
comment-plan is plan-only
comment-plan has <= 3 candidates
comment-plan references known cards
comment-plan not_selected entries reference known cards
comment-plan not_selected entries do not repeat planned comments
comment-plan has no duplicate card IDs or duplicate path/line anchors
comment-plan has renderable line/path fields
comment-plan carries structured next action, actionability, relevance, and witness route fields
comment-plan comment bodies stay within the hard 220-word limit
comment-plan includes trust boundary
witness-plan includes route limits
lsp.json contains read-only projections
lsp.json code actions are command-only
no WorkspaceEdit appears
no positive overclaim wording appears
```

The checker validates the first-pr bundle, parses machine-readable artifacts,
confirms advisory policy, verifies comment-plan plan-only behavior, checks card
IDs, keeps counts consistent, requires witness route limits, and requires trust
boundary presence.

## 9. Overclaim rejection

CI must reject public artifacts that say or imply:

```text
safe
sound
verified
proved
UB-free
Miri-clean
all clear
site reached
test covered this unsafe site
blocking-ready
calibrated precision
calibrated recall
```

unless those terms appear only in explicit negative/trust-boundary wording,
such as:

```text
This does not prove the repo safe.
This is not UB-free status.
This is not a Miri result.
```

This applies to:

```text
README badge text
badge endpoint JSON
pr-summary.md
comment-plan.json
witness-plan.md
lsp.json
policy report
outcome report
GitHub job summary
release notes
publication receipts
```

## 10. PR summary / GitHub summary contract

The first-pr workflow should write a GitHub job summary.

Minimum shape:

```markdown
## unsafe-review advisory summary

Artifacts verified.

Cards:
- Total: N
- Actionable: N
- Suppressed: N
- Baseline-known: N

Top card:
- `UR-...`
- Operation: `raw_pointer_read`
- Missing: alignment evidence
- Route: Miri / cargo-careful

Open:
- `target/unsafe-review/pr-summary.md`
- `target/unsafe-review/witness-plan.md`

Trust boundary:
Static unsafe contract review only. Not memory-safety proof, not UB-free status,
not Miri-clean status, and not site-execution proof.
```

If no changed gaps:

```markdown
## unsafe-review advisory summary

Artifacts verified.

No changed unsafe-review gaps were found.

This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.
```

## 11. Comment-plan CI behavior

The CI workflow may generate:

```text
comment-plan.json
```

It must not post it by default.

Comment-plan constraints:

```text
max 3 candidates
changed lines only
high-confidence actionable cards only
no static_unknown
no operation_family unknown
no baseline-known
no suppressed
no posting by default
```

If comment-plan verification fails, CI may fail because the artifact contract is
broken.

If comment-plan contains zero candidates, CI should still pass.
When review cards exist but no inline candidate is selected, `not_selected`
entries should explain why the card stayed out of the inline comment budget.

## 12. Witness tool policy

Default CI must not run:

```text
Miri
cargo-careful
ASan
MSan
TSan
LSan
Loom
Shuttle
Kani
Crux
fuzzing
mutation testing
```

Default CI may route to those tools in `witness-plan.md`.

Witness execution belongs to:

```text
targeted PR lane
nightly lane
release readiness lane
manual local user action
```

A witness receipt may be imported only through explicit receipt surfaces. CI
must not fabricate receipts.

## 13. Coverage / Codecov design

Codecov belongs in a separate advisory lane.

Recommended workflow posture:

```text
cargo-llvm-cov
LCOV output
Codecov upload
fail_ci_if_error: false initially
```

Recommended workflow shape:

```yaml
name: Coverage

on:
  pull_request:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read

jobs:
  coverage:
    runs-on: ubuntu-latest
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v6
        with:
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@1.95.0
      - name: Install cargo-llvm-cov
        uses: taiki-e/install-action@cargo-llvm-cov
      - name: Generate LCOV
        run: |
          cargo llvm-cov --workspace --all-targets --locked \
            --lcov \
            --output-path target/llvm-cov/lcov.info
      - name: Upload to Codecov
        uses: codecov/codecov-action@v5
        with:
          files: target/llvm-cov/lcov.info
          fail_ci_if_error: false
```

The workflow must not become a release gate or an unsafe-correctness signal
without a separate accepted policy change.

Badge posture:

```text
Codecov = Rust test execution-surface telemetry
not unsafe correctness
not memory-safety proof
not UB-free status
not witness adequacy
```

## 14. Source/swarm CI routing

CI design must respect repo roles.

```text
unsafe-review-swarm:
  routine implementation, analyzer, evidence, dogfood, LSP/agent, CI experiments

unsafe-review:
  source of record, curated promotions, release prep, publication receipt, package metadata
```

Source repo CI should remain quieter and release-focused.

Swarm CI may carry more experimental lanes.

Every direct source PR must declare whether it is:

```text
swarm-originated promotion
direct public/release surface
urgent source hotfix
source-only repo hygiene
```

The source/swarm promotion policy exists to prevent routine implementation from
drifting into the source repo and to keep source as the public release surface.

## 15. Branch protection and merge behavior

CI design must distinguish:

```text
quality failure
configuration obstacle
agent runtime state
```

A green PR blocked only by a single-contributor external-review branch policy is
a configuration obstacle, not a code quality finding.

Agent runtime state is never PR state.

CI and PR policies must not close, supersede, park, or mutate PRs because:

```text
Codex session is busy
agent cap was hit
another PR is active
current branch differs
```

Runtime/session state is a handoff fact, not a repository fact.

## 16. Example default workflow

The default workflow shape is:

```yaml
name: CI

on:
  pull_request:
  push:
    branches: [main, master]
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}

jobs:
  rust:
    runs-on: ubuntu-latest
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v6
        with:
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@1.95.0
        with:
          components: rustfmt, clippy
      - run: cargo fmt --check
      - run: cargo check --workspace --all-targets --locked
      - run: cargo clippy --workspace --all-targets --locked -- -D warnings
      - run: cargo test --workspace --all-targets --locked
      - run: cargo doc --workspace --no-deps --locked
        env:
          RUSTDOCFLAGS: -D warnings
      - run: cargo run --locked -p xtask -- check-pr
```

## 17. Example advisory first-pr workflow

The first-pr workflow shape is:

```yaml
name: unsafe-review first-pr

on:
  pull_request:
    types: [opened, reopened, synchronize, ready_for_review]
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: unsafe-review-first-pr-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}

jobs:
  first_pr_bundle:
    name: unsafe-review advisory packet
    if: ${{ github.event_name == 'workflow_dispatch' || github.event.pull_request.draft == false }}
    runs-on: ubuntu-latest
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@1.95.0
      - run: cargo build --locked -p unsafe-review
      - name: Render first-pr advisory bundle
        env:
          BASE_REF: ${{ github.base_ref || github.event.repository.default_branch }}
        run: |
          mkdir -p target/unsafe-review
          ./target/debug/unsafe-review first-pr \
            --base "origin/${BASE_REF}" \
            --out-dir target/unsafe-review
      - name: Verify first-pr artifact contract
        run: cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review
      - name: Write GitHub summary
        run: |
          {
            echo "## unsafe-review advisory summary"
            echo
            cat target/unsafe-review/pr-summary.md
            echo
            echo
            echo "> Trust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, not Miri-clean status, and not site-execution proof."
          } >> "$GITHUB_STEP_SUMMARY"
      - uses: actions/upload-artifact@v7
        if: always()
        with:
          name: unsafe-review-first-pr
          path: |
            target/unsafe-review/cards.json
            target/unsafe-review/pr-summary.md
            target/unsafe-review/cards.sarif
            target/unsafe-review/comment-plan.json
            target/unsafe-review/witness-plan.md
            target/unsafe-review/lsp.json
          if-no-files-found: error
```

## 18. CI proof

This spec is satisfied when these pass locally and in CI:

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo run --locked -p xtask -- check-pr
```

First-pr artifact proof:

```bash
cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --out-dir target/unsafe-review-first-pr-smoke

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-first-pr-smoke
```

No-card artifact proof:

```bash
cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/safe_code_no_cards \
  --diff fixtures/safe_code_no_cards/change.diff \
  --out-dir target/unsafe-review-no-card-smoke

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-no-card-smoke
```

Source/swarm proof:

```bash
cargo run --locked -p xtask -- source-divergence
```

Release proof:

```bash
cargo package -p unsafe-review-core --list
cargo package -p unsafe-review-cli --list
cargo package -p unsafe-review --list
cargo publish -p unsafe-review-core --dry-run
cargo publish -p unsafe-review-cli --dry-run
cargo publish -p unsafe-review --dry-run
```

## 19. Acceptance examples

### Example A - card found, CI passes

Input:

```text
PR changes a raw pointer read.
unsafe-review emits one guard_missing card.
```

Expected:

```text
workspace CI passes
first-pr bundle verifies
GitHub summary shows advisory card
CI passes
```

Reason:

```text
findings are advisory by default
```

### Example B - malformed artifact, CI fails

Input:

```text
comment-plan.json references unknown card_id
```

Expected:

```text
check-first-pr-artifacts fails
first-pr lane fails
```

Reason:

```text
artifact integrity failure
```

### Example C - no cards, CI passes

Input:

```text
PR has no changed unsafe-review gaps
```

Expected:

```text
first-pr bundle verifies
summary says no changed unsafe-review gaps were found
summary says this does not prove safety / UB-free / Miri-clean / site execution
CI passes
```

### Example D - Codecov upload fails, CI passes initially

Input:

```text
Codecov upload flakes
```

Expected initial behavior:

```text
coverage workflow reports warning/failure in advisory lane
default workspace CI unaffected
release not blocked
```

Later policy may change this, but not by default.

### Example E - source drift detected, swarm work pauses

Input:

```text
source has new implementation commits not mirrored into swarm
```

Expected:

```text
source-divergence reports new_source_commits
routine feature work pauses
sync/ack PR is opened in swarm
```

## 20. Lifecycle status

This spec is accepted because the repository now has:

```text
SPEC-0024 exists and is linked from the spec index
docs/ci/PR_CI.md matches SPEC-0024
default CI uses read-only permissions
first-pr example workflow exists
first-pr artifact verifier is documented
comment-plan remains plan-only
no workflow posts comments by default
no workflow runs witnesses by default
source-divergence is documented
release-readiness commands are documented
```

Move to release-backed when:

```text
0.2.x publication receipt records install / first-pr / support smoke
first-pr bundle verification passes in CI
source/swarm sync guard is used in swarm
coverage lane, if present, has advisory wording and first successful upload
```
