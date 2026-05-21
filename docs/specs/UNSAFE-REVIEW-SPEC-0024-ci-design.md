# UNSAFE-REVIEW-SPEC-0024: CI design

Status: draft
Owner: repo-infra / ci
Created: 2026-05-21

Linked specs:
- UNSAFE-REVIEW-SPEC-0011: PR and CI output
- UNSAFE-REVIEW-SPEC-0012: LSP and editor projection
- UNSAFE-REVIEW-SPEC-0019: First-run cockpit
- UNSAFE-REVIEW-SPEC-0020: Source-of-truth stack
- UNSAFE-REVIEW-SPEC-0022: PR commenting experience
- UNSAFE-REVIEW-SPEC-0023: First-hour experience

Linked docs:
- docs/ci/PR_CI.md
- .github/workflows/ci.yml
- .github/examples/unsafe-review-first-pr.yml
- docs/contributing/SWARM_TO_MAIN.md
- docs/BADGE_POLICY.md

Support-tier impact:
- docs/status/SUPPORT_TIERS.md

Policy impact:
- policy/ci-lane-whitelist.toml
- policy/doc-artifacts.toml
- policy/package-boundary.toml

## 1. Purpose

`unsafe-review` CI must protect two things at the same time:

```text
the Rust workspace
the advisory unsafe-review artifact contract
```

CI must make the repo safe to maintain and useful to users without overstating what the tool proves.

The default CI design is:

```text
cheap deterministic workspace checks
+ advisory first-pr packet verification
+ no witness execution by default
+ no automatic comments by default
+ no blocking on unsafe-review findings by default
```

CI proves that the tool and artifacts are well-formed. It does **not** prove the reviewed Rust code is safe.

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

The current PR/CI model already encodes the key rule: the PR gate fails on infrastructure and contract failures, not advisory unsafe-review findings by default.

## 3. Current default CI contract

The default workflow runs on:

```text
pull_request
push to main/master
workflow_dispatch
```

It uses read-only repository permissions, cancels superseded pull request runs, disables persisted checkout credentials, and bounds the Rust job with a timeout. The current workflow uses `contents: read`, `persist-credentials: false`, `cancel-in-progress` for pull requests, and a 20-minute Rust workspace timeout.

The default workspace gate is:

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo run --locked -p xtask -- check-pr
```

That is the baseline.

## 4. CI lane taxonomy

### 4.1 `ci.yml` — default workspace gate

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

### 4.2 `unsafe-review-first-pr.yml` — advisory PR packet lane

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

The current example workflow already follows this shape: it builds `unsafe-review`, runs `first-pr`, verifies the bundle, writes a GitHub summary, and uploads the first-pr artifacts.

May fail on artifact contract violations and overclaims, but not on advisory findings by default.

### 4.3 `coverage.yml` — advisory coverage / Codecov lane

Coverage is telemetry for Rust test execution surface, not unsafe correctness proof. This lane should start advisory (`fail_ci_if_error: false`) with no threshold gate.

### 4.4 `release-readiness.yml` — manual release proof lane

Manual release readiness commands may include package list, publish dry-run, install smoke, first-pr smoke, and support smoke. This lane must not publish by itself without a dedicated owner-approved workflow.

### 4.5 `source-divergence.yml` or local-only sync guard

Source/swarm sync is checked with:

```bash
cargo run --locked -p xtask -- source-divergence
```

Default behavior is advisory at first.

### 4.6 Future `comment-poster.yml` — trusted poster lane

Any write-token comment poster must verify artifacts and consume `comment-plan.json` in a separate trusted workflow that does not run PR-controlled code before posting.

## 5. CI permissions policy

Default workspace and first-pr lanes use:

```yaml
permissions:
  contents: read
```

Grant write scopes only in explicitly trusted workflows.

## 6. Checkout and token posture

All workflows should prefer:

```yaml
- uses: actions/checkout@v6
  with:
    persist-credentials: false
```

except for explicitly scoped trusted commit-back flows.

## 7. Toolchain posture

CI should install pinned Rust 1.95.0 and required components (`rustfmt`, `clippy`).

## 8. Artifact integrity checks

The first-pr artifact checker is a CI gate:

```bash
cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review
```

It must verify machine-readable artifact parseability and cross-artifact consistency, plan-only comment-plan behavior, trust-boundary presence, and no positive overclaim wording.

## 9. Overclaim rejection

Public artifacts must reject positive safety/proof claims (safe/sound/verified/UB-free/Miri-clean/all clear) unless they appear in explicit negative trust-boundary wording.

## 10. PR summary / GitHub summary contract

First-pr workflows should publish concise advisory summary content and always include trust-boundary wording.

## 11. Comment-plan CI behavior

`comment-plan.json` may be generated and verified in CI, but not posted by default.

## 12. Witness tool policy

Default CI routes witness tools in `witness-plan.md`; it does not execute witness tools by default.

## 13. Coverage / Codecov design

Coverage belongs in a separate advisory lane and does not prove unsafe correctness, memory safety, UB freedom, or witness adequacy.

## 14. Source/swarm CI routing

`unsafe-review-swarm` carries routine development lanes; `unsafe-review` remains the release/public source-of-record lane.

## 15. Branch protection and merge behavior

CI design distinguishes quality failures from configuration obstacles and agent runtime state.

## 16. Example default workflow

See `.github/workflows/ci.yml` and `docs/ci/PR_CI.md` for the current default shape and required checks.

## 17. Example advisory first-pr workflow

See `.github/examples/unsafe-review-first-pr.yml` for the advisory packet lane and trust-boundary summary model.

## 18. CI proof

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo run --locked -p xtask -- check-pr
cargo run --locked -p xtask -- source-divergence
```

## 19. Acceptance examples

- Card found, artifacts valid: CI passes (advisory finding).
- Artifact malformed: checker fails and CI fails.
- No cards found: CI passes with no-card honesty text.
- Coverage upload failure: advisory lane may warn/fail without blocking default workspace CI.
- Source drift found: swarm routine work pauses for sync/ack handling.

## 20. Promotion rule

Promote to accepted when SPEC-0024 is indexed, docs align, default lanes enforce read-only posture, first-pr verifier is documented and used, no default comment posting/witness execution is preserved, and source-divergence/release-readiness expectations are documented.
