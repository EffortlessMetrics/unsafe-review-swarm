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

The CI control-plane line:

```text
xtask is the repo-facing policy surface.
Upstream tools are engine-room substrates.
```

CI policy must be encoded in the cargo-allow spec-system graph, `xtask`, policy
ledgers, and source-of-truth docs instead of scattering repository authority
across workflow YAML, one-off shell scripts, or direct upstream-tool
invocations. Cargo-allow owns proposal/spec/plan/active-goal/support-tier/
closeout linkage; `xtask` owns implementation and policy proof. Workflows may
invoke both tools, but each lane must name what is being proved, when it runs,
and which claims it does not make.

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

### 3.1 Upstream substrate policy

The repo standardizes on a small upstream substrate set, but keeps repository
authority behind `xtask` and ReviewCard-derived artifacts. The current substrate
map is:

| Plane | Upstream substrate | Repo-facing surface | Default CI posture |
| --- | --- | --- | --- |
| Syntax and codemod candidates | `ast-grep`; Rust-specific authority through Rust-aware syntax data such as rust-analyzer crates | `xtask` policy checks, analyzer code, ReviewCard projections | Candidate generation only; not final Rust identity authority |
| Workspace graph | `cargo_metadata`; `guppy` when richer graph queries are justified | `xtask` lane planning, package-boundary checks, release planning | Allowed when wrapped by repo policy |
| Test execution | pinned `cargo-nextest 0.9.143` through `xtask ci-test`, with `cargo test --workspace --doc --locked` parity | `xtask ci-test` and bounded `test-diagnostics.json` | Deterministic test result; doctests remain an explicit part of the combined result |
| Coverage | `cargo-llvm-cov` and Codecov | coverage lane and CI-lane ledger | Advisory telemetry only |
| Static mutation exposure | `ripr` | future explicit `xtask`/artifact lane | Candidate weak-oracle signal; not killed/survived mutation proof |
| Runtime mutation | `cargo-mutants` | targeted, nightly, or release lane | Not default PR full-workspace tax |
| Unsafe contract review | `unsafe-review` | ReviewCards and first-pr bundle | Advisory by default |
| Runtime UB witness | Miri and other witness tools | witness routes and imported receipts | Not default CI execution |
| Source exceptions | `cargo-allow` | policy ledgers and receipts when introduced | Exception evidence, not broad suppression |
| Dependency trust | `cargo-deny`; later `cargo-vet`; RustSec/`cargo-audit` where appropriate | future supply-chain wrapper and policy ledger | Dependency policy only; no unsafe-correctness claims |
| Public API / release | `cargo-semver-checks`; rustdoc JSON for custom inventories | release-readiness lane | Release/manual proof, not ordinary PR cost |
| Workflow policy | `actionlint`; `zizmor` | workflow allowlist, CI-lane whitelist, `xtask` checks | Correctness/security linting without expanding tokens |
| Text and config hygiene | `taplo`; `typos`; markdown/link tooling | docs/policy wrappers | Advisory until baselined, then explicitly gated |

Substrate adoption rules:

```text
ast-grep finds syntactic candidates; Rust-aware tooling decides Rust identity.
cargo_metadata/guppy describe the workspace; xtask decides CI routing.
cargo-nextest may run tests; `xtask ci-test` resolves only the pinned, hashed
asset and keeps `core_exit` authoritative. The wrapper retains no runner
stdout/stderr and runs doctests separately when nextest does not execute them.
cargo-llvm-cov measures execution surface; it does not prove correctness.
ripr shifts mutation signal left; cargo-mutants remains the runtime backstop.
unsafe-review makes unsafe changes reviewable; Miri provides concrete witness evidence only when run and receipted.
cargo-deny/vet/audit own dependency trust; they do not review unsafe contracts.
cargo-semver-checks owns release API compatibility; it is not a default PR witness.
```

A new upstream tool may be added to workflow YAML only when the PR also records
its repo-facing surface, trigger policy, artifact policy, cost posture, and
claim boundary in the relevant spec or policy ledger. The live structured
runner pin is:

```text
cargo-nextest 0.9.143
x86_64-unknown-linux-gnu archive sha256:
66786b9abe23920d022a182d1416b1bbc8130dd4872a9553d76985a1708dcd1e
x86_64-pc-windows-msvc archive sha256:
c42a1dbde532da06dc9b4a43d44fd0ce668b836c2ab7388410f10ff9834476a2
```

The wrapper resolves the binary under `target/ci-tools`, verifies the archive
before extraction, rejects symlinked paths, and invokes `nextest run` with its
supported JUnit reporter with failure output disabled. Only bounded failed-test
package/name pairs survive into the diagnostic projection. Unexpected,
malformed, overlong, control-character,
secret-like, PEM-like, duplicate, or over-limit records are dropped or mapped
to a fixed status. This proves only a structured failed-test-result surface; it
does not prove root cause, PR causality, safety, UB-freedom, Miri cleanliness,
or site execution.

## 4. CI lane taxonomy

Every live or planned CI lane must have a named purpose. High-cost or
write-token lanes must not be folded into the default workspace gate by
convenience.

### 4.1 `ci.yml` - single tight CI gate

Purpose:

```text
protect the Rust workspace and repo policy checks with one tight gate, and
layer advisory LLM review on top without expanding the hard gate
```

The live swarm `ci.yml` is one tight CI gate, not a pile of parallel required
checks. It is a single gate job whose runner is chosen by a minimal capacity
router (self-hosted primary, `ubuntu-latest` overflow — see section 7); the
router is advisory and not a required check, so there is still exactly one job
that gates the merge and one required status check. The gate job carries only
the deterministic layer; the advisory LLM layer runs as its own standalone
workflow:

```text
mandatory deterministic core floor (the hard gate, ci.yml)
  cargo run --locked -p xtask -- check-pr

advisory LLM layer (standalone non-blocking workflow, ub-review.yml)
  EffortlessMetrics/ub-review review (gh-runner profile)
```

The deterministic core floor is the only hard blocker and the only required
status check. ub-review wraps it as an additive layer: it does not replace the
deterministic tools, it reviews on top of their evidence in a separate
workflow that never gates the merge (see "Standalone advisory ub-review lane"
below). The value is strong gating from a tight central set plus advisory LLM
review that costs the gate nothing.

Single required check:

```text
The job is named "Unsafe Review Rust Result" for branch-protection continuity.
Its conclusion reflects ONLY the deterministic core verdict (the final assert
step fails iff `xtask check-pr` exited non-zero). ub-review runs in a separate
advisory workflow and can never flip that result.
```

Step shape inside the one gate job:

```text
0. route (advisory, ubuntu-latest, not a required check): pick the gate runner —
   idle trusted self-hosted em-ci runner else ubuntu-latest overflow — and emit a
   runs-on value plus runner_kind (see section 7)
1. shared setup once: checkout (fetch-depth 0), dtolnay/rust-toolchain@1.95.0
   with rustfmt + clippy, Swatinem/rust-cache@v2
2. fast precontext: runner-kind-aware disk/scratch handling, then cargo fmt
   --check plus repo/PR facts written to target/ci-core/precontext.md as a
   durable run record, and the core gate launched in the background sharing the
   workspace target dir (cargo's target-lock serialises overlap safely); diff
   scoping reads the quoted runner-provided `GITHUB_BASE_REF` with a `main`
   default, and an unavailable base comparison forces the full test path
3. final assert: wait for the current run/attempt's background core gate,
   surface its closed-vocabulary step-status summary, and fail the job iff its
   exit code != 0; stale `core_exit` files are removed before launch and cannot
   satisfy the run-keyed wait
4. failure evidence: when the core exit is non-zero, create only a bounded,
   allowlisted step-status summary, machine-readable command/commit/exit/path
   metadata, and (when the structured test step produced it) the exact regular
   `test-diagnostics.json` projection; always attempt a non-fatal upload with
   seven-day retention and never read or upload the raw core log
```

The failure-evidence upload does not create a second lane or verdict. The final
assert remains the only authority: artifact preparation or upload failure must
not make a failed core gate pass, and artifact upload must not make a passing
core gate fail. On success no evidence paths are exposed, so the
`always()`-guarded upload step is skipped. After a non-zero verdict, staging is removed
and recreated as a private unpredictable directory under the per-job
`RUNNER_TEMP`: cleanup is followed immediately by `mkdir -m 700` for the
private parent and then `mktemp -d` for unpredictable staging. The upload action receives only the exact `summary.md` and
`metadata.json`, and optional `test-diagnostics.json` regular-file paths, never
a workspace or staging directory. `test-diagnostics.json` is capped at 16 KiB
and contains no raw output, panic text, backtrace, source snippet, environment
value, secret, or arbitrary path. It is a diagnostic projection only and can
never alter the run/attempt-keyed `core_exit` verdict. The workflow invokes
`cargo run --locked -p xtask -- ci-test-validate <path>` immediately after the
private copy and before exposing the optional path as an upload output; that
validator rejects schema drift, malformed JSON, unknown fields, authority drift,
unsorted/duplicate records, hostile values, and symlinks.
Pre-populated workspace extras and `core.log` symlinks are therefore outside the
artifact selection. The intermediate closed-vocabulary step-status excerpt is
also created atomically inside that private staging directory and verified as a
regular non-symlink before any read; the workflow never uses the predictable
workspace `target/ci-core/redacted-excerpt.txt` path, and a failed write or move
suppresses evidence outputs rather than falling through to a stale read. The retained status summary
accepts only the fixed step ids `fmt`, `clippy`, `test`, and `check-pr`, decimal
timing/exit fields, and the literal `skipped`; it is capped at 80 lines and 16
KiB. Arbitrary stdout/stderr, bare tokens, PEM blocks, and other `core.log`
content have no projection into the artifact.

This same-job design cannot fully isolate staging from a deliberately persistent
hostile process running as the runner user after the core command returns. The
fresh unpredictable runner-temp directory, owner-only permissions, atomic file
replacement, regular-file/no-symlink checks, and exact two-or-three-file upload selection
bound that residual risk. Strong isolation would require a separately trusted
job that downloads and validates a prior artifact, which is outside this
diagnostic-only lane.

Standalone advisory ub-review lane (`.github/workflows/ub-review.yml`):

```text
- pull_request (opened, reopened, ready_for_review, synchronize), same-repo
  non-draft PRs only: fork PRs cannot read the MINIMAX_API_KEY org secret,
  drafts would burn advisory LLM budget early, and the deterministic core
  gate still runs for forks and drafts
- superseded runs are cancelled per PR (concurrency cancel-in-progress) so
  rapid pushes do not stack redundant paid reviews
- the EffortlessMetrics/ub-review action is pinned to an immutable commit SHA
  (gh-runner profile, posting: review, fail-on-gate 'false')
- the job is continue-on-error with a bounded timeout, and no
  branch-protection rule names this workflow: it is NEVER a required check,
  so LLM availability or opinion can never block the merge
- workflow-level permissions are contents: read; pull-requests: write is
  granted at the job level only, solely so ub-review can post its grouped
  advisory PR review; the run also uploads its artifact bundle
- pin bumps update policy/workflow-allowlist.toml in the same PR (a dependabot
  pin bump alone cannot pass the deterministic gate, because the allowlist
  pins the action SHA)
- xtask check-ci-routing-contract enforces this shape: the advisory posture
  markers must stay present in ub-review.yml, and an in-job ub-review step in
  ci.yml is a forbidden marker (it would double-run the advisory review)
```

May fail on:

```text
repo policy failure surfaced by xtask check-pr
```

The deterministic core gate (`xtask check-pr`) is the merge-blocking floor.
`cargo fmt --check` runs as advisory precontext only; full clippy, test, and
rustdoc proof stay in the local validation loop and release lanes, as in section
3. ub-review findings, ub-review gate manifest conclusions, and ub-review/model
availability never fail the merge.

Must not run as part of the hard gate:

```text
Miri
cargo-careful
sanitizers
Loom
Kani
mutation testing
source edits
publish
```

The advisory ub-review lane may post one grouped PR review (posting:review),
but it must not edit source, run witnesses, publish, or make blocking
unsafe-correctness claims.

Gate job permissions:

```yaml
permissions:
  contents: read
```

The gate job grants no write token. `pull-requests: write` lives only in the
standalone advisory ub-review workflow, for one reason only: so ub-review can
post its grouped advisory PR review.

### 4.2 `policy-contracts.yml` - source-of-truth gate

Purpose:

```text
protect spec, policy, package-boundary, docs-automation, goal, and CI-lane ledgers
```

Runs:

```text
cargo-allow doctor --profile spec-system
cargo-allow check --profile spec-system --mode audit
cargo-allow worklist --profile spec-system --format json
check-doc-artifacts
check-docs-automation
check-package-boundary
check-ci-lanes
check-policy
```

During the swarm CI budget window, pull-request runs are path-scoped to
source-of-truth artifacts and the legacy parity snapshot:

```text
policy/**
.allow/**
.rails/**
plans/**
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
target/unsafe-review/review-kit.json
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/github-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/receipt-audit.md
target/unsafe-review/receipt-audit.json
target/unsafe-review/policy-report.json
target/unsafe-review/policy-report.md
target/unsafe-review/manual-candidates.json
target/unsafe-review/manual-repair-queue.json
target/unsafe-review/tokmd-packets.json
target/unsafe-review/usefulness-telemetry.json
target/unsafe-review/lsp.json
target/unsafe-review/repair-queue.json
target/unsafe-review/unsafe-review-gate.json
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

Posture (2026-06-21): the guard runs **local-only** by design. A hosted
runner would have to `git fetch` the same two repos into the local refs
the guard reads (`refs/unsafe-review-sync/source-main`,
`refs/unsafe-review-sync/swarm-main`), so a CI workflow would not produce
information a developer does not already get faster from
`cargo run --locked -p xtask -- source-divergence` locally. The guard
remains advisory (`Ok(())` always — `xtask/src/source_sync.rs`), never a
hard CI failure, and is not wired into any workflow. Developers run it
before routine swarm implementation per `AGENTS.md`. If the owner later
wants on-demand CI invocation without a local checkout, a
`workflow_dispatch`-only workflow is the documented path; it is not
warranted today. See #1809.

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

### 4.8 `droid-pr-review.yml` - advisory Droid PR review bot

Purpose:

```text
run an owner-requested Factory Droid advisory review on non-draft pull requests
using the MiniMax M3 BYOK custom model
```

The workflow configures `~/.factory/settings.json` with a `customModels` entry
for `MiniMax-M3` through MiniMax's Anthropic-compatible endpoint, clears ambient
Anthropic globals before invoking Droid, and passes `custom:MiniMax-M3-0` for
both the review and security-review model. The advisory review uses Droid's
shallow preset with low reasoning effort so routine PR runs stay bounded.
Droid security review blocking is disabled; this lane may report advisory
findings, but it must not submit blocking security review outcomes by default.

The workflow passes the scoped workflow GitHub token to Droid explicitly. The
pinned Factory Droid action's review validator also requests an OIDC token after
the review pass, so this lane grants `id-token: write` for that validator path
only; it must not grant `issues: write` or `actions: read`.

The Droid action step uses its own timeout and `continue-on-error` so review-bot
latency, model failures, or validator failures do not become default merge
blocking. The job has a slightly longer timeout only to allow the workflow to
complete cleanup after the bounded Droid step.

This lane is bounded with a job timeout and grants only:

```yaml
permissions:
  contents: read
  pull-requests: write
  id-token: write
```

This lane may post advisory Droid review comments, but it must not run
unsafe-review witnesses, edit source, publish artifacts, or make blocking
unsafe-correctness claims. It remains separate from the unsafe-review
ReviewCard-derived first-pr lane and does not change the default rule that
unsafe-review findings are advisory by default.

The workflow is registered in:

- [policy/workflow-allowlist.toml](../../policy/workflow-allowlist.toml)
- [policy/ci-lane-whitelist.toml](../../policy/ci-lane-whitelist.toml)

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

Runner posture: self-hosted primary with GitHub-hosted overflow. A minimal
`route` job (NOT a required check) decides where the single tight gate runs. It
reads org runner state via `gh api orgs/EffortlessMetrics/actions/runners` using
the `EM_RUNNER_READ_TOKEN` secret (a secret on a `run:` step, not a new external
action) and emits a `runs-on` value plus a `runner_kind`:

```text
self-hosted (primary): an idle trusted runner exists in the owned em-ci fleet —
  online, !busy, carrying the shared self-hosted/linux/x64/em-ci/trusted-pr
  label set. The router matches the shared group labels only (any idle size
  catches it); it never size-routes to cpx42/cx43/cx53. The gate consumes a JSON
  label array via fromJSON(needs.route.outputs.runner).

github (overflow): no idle trusted self-hosted capacity, missing runner-read
  token, org-runner API failure, or a fork PR. The router emits the JSON string
  "ubuntu-latest"; the gate consumes it via the same fromJSON wiring (a JSON
  string and a JSON array are both valid runs-on shapes).
```

The owned fleet absorbs the bulk; gh-hosted only handles bursts, capacity gaps,
and forks. Fork PRs always overflow to gh-hosted (untrusted code can never run
on trusted self-hosted runners); the standalone advisory ub-review workflow is
separately guarded to same-repo PRs (no org secrets for forks), and the
deterministic core gate still runs for forks. There
is still exactly ONE job that gates the merge and ONE required check
(`Unsafe Review Rust Result`); the router is advisory and never blocks. The gate
branches disk/scratch handling on `runner_kind`: on gh-hosted overflow it frees
the big preinstalled SDKs (android/dotnet/ghc/CodeQL) when headroom is low; on
self-hosted it leaves those gh-only paths alone, reports `df -h` headroom, and
reuses the shared workspace target dir (cargo's target-lock serialises overlap),
with shared-fleet scratch hygiene tracked in unsafe-review-swarm #1519.

Advisory LLM layer cost posture: ub-review runs in the standalone
`ub-review.yml` workflow on `ubuntu-latest` with the `gh-runner` profile and
the `MINIMAX_API_KEY` org secret. It is bounded by its own job timeout and is
advisory (`fail-on-gate: 'false'`, `continue-on-error`, never a required
check), so it never blocks the merge and its runtime never extends the
deterministic gate's wall-clock.

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
cards.json schema_version is checked
pr-summary.md exists
cards.sarif exists and parses
comment-plan.json exists and parses
comment-plan.json schema_version is checked
witness-plan.md exists
lsp.json exists and parses
lsp.json schema_version is checked
manual-candidates.json exists and parses
manual-candidates.json schema_version is checked
manual-repair-queue.json exists and parses
manual-repair-queue.json schema_version is checked
manual-repair-queue.json stays source = manual_candidate and policy = advisory
manual-repair-queue.json queue entries preserve manual-candidate markers and
copy-only guidance from manual-candidates.json
manual-repair-queue.json optional stable-byte seed source/count metadata and
per-entry seed rows are checked against manual candidate IDs when present
manual-repair-queue.json does not claim analyzer discovery, automatic repair,
agent execution, witness execution, source edits, comments, proof, or policy
tokmd-packets.json exists and parses
tokmd-packets.json schema_version is checked
tokmd-packets.json stays source = first_pr and policy = advisory
tokmd-packets.json packets preserve manual-candidate markers and copy-only
formatting inputs from manual-candidates.json
tokmd-packets.json records absent ledger, receipt, and ReviewCard packet inputs
instead of rendering tokmd output
gating
repair-queue.json exists and parses
policy-report.json exists and parses
policy-report.json schema_version is checked
policy-report.md exists

card IDs align across artifacts
result counts stay internally consistent
comment-plan is plan-only
comment-plan has <= 3 candidates
comment-plan references known cards
comment-plan accounts for every ReviewCard in comments[] or not_selected[]
comment-plan not_selected entries reference known cards
comment-plan not_selected entries do not repeat planned comments
comment-plan not_selected entries project ReviewCard operation and next action
comment-plan has no duplicate card IDs or duplicate path/line anchors
comment-plan has renderable line/path fields and keeps selected comments changed-line scoped
comment-plan carries structured next action, actionability, relevance, and witness route fields
comment-plan comment bodies stay within the hard 220-word limit
comment-plan includes trust boundary
manual-candidate markers are rejected from ReviewCard-only first-pr artifacts:
cards.json, cards.sarif, comment-plan.json, lsp.json, repair-queue.json,
policy-report.json, and policy-report.md
manual-candidate markers are allowed only in manual-candidates.json,
manual-repair-queue.json, and the review-kit manual-candidate handoff
manual-candidate reviewcard_artifact_applicability marks ReviewCard-only
artifacts as not applicable to manual candidates and rejects marker allowance
policy-report artifacts remain ReviewCard-only policy simulation and
exclude manual candidates as policy inputs
witness-plan includes route limits
receipt-audit.md exists
receipt-audit includes saved-receipt metadata summary and trust boundary
receipt-audit does not claim witness execution, site execution, proof, or safety
lsp.json contains read-only projections
lsp.json code actions are command-only
repair-queue.json schema_version is checked
repair-queue.json references known ReviewCards
repair-queue.json bucket names use the closed vocabulary
repair-queue.json bucket reasons use the closed vocabulary
repair-queue.json buckets do not repeat a ReviewCard
repair-queue.json readiness state uses the closed vocabulary and matches readiness
repair-queue.json readiness reasons are present
pr-summary.md top-card agent handoff line projects repair-queue.json readiness
state, buckets, bucket reasons, and readiness reasons
github-summary.md top-card agent handoff line projects repair-queue.json
readiness state, buckets, bucket reasons, and readiness reasons
repair-queue.json entries carry do-not-do boundaries
repair-queue.json entries preserve the ReviewCard-derived typed repair_candidates array
repair-queue.json human-review and do-not-auto-repair entries are not agent-ready
repair-queue.json does not claim agent execution or repair success
review-kit.json handoff.review_cards has a bounded card_queue with limit and
omitted-card counts
review-kit.json handoff.review_cards entries reference known ReviewCards only
review-kit.json handoff.review_cards entries project cards.json identity,
location, operation, missing evidence, and next action
review-kit.json handoff.review_cards entries project cards.json verify commands
and witness routes
review-kit.json handoff.review_cards entries project repair-queue.json buckets,
bucket reasons, and agent-readiness state
review-kit.json handoff.review_cards entries preserve repair-queue.json typed
repair_candidates without reclassification
review-kit.json handoff.review_cards stays ReviewCard-only and excludes manual
candidate marker fields
review-kit.json handoff.review_cards carries copy-only trust boundary wording
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
cards.json
pr-summary.md
github-summary.md
cards.sarif
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
- `target/unsafe-review/receipt-audit.md`
- `target/unsafe-review/receipt-audit.json`
- `target/unsafe-review/policy-report.json`
- `target/unsafe-review/policy-report.md`
- `target/unsafe-review/unsafe-review-gate.json`

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

A drop-in default workflow can run the full deterministic proof set directly:

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

The live swarm `ci.yml` instead uses the single tight gate of section 4.1: one
gate job whose mandatory deterministic floor is
`cargo run --locked -p xtask -- check-pr` (the only required check, named
"Unsafe Review Rust Result"). Its shape is:

```yaml
jobs:
  unsafe-review-rust-result:
    name: Unsafe Review Rust Result
    runs-on: ubuntu-latest
    timeout-minutes: 60
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@1.95.0
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: Fast precontext and launch core gate
        run: |
          # cargo fmt --check + repo/PR facts -> target/ci-core/precontext.md,
          # then launch `cargo run --locked -p xtask -- check-pr` in the
          # background on the shared workspace target dir.
          ...
      - name: Assert core gate verdict
        id: core-verdict
        if: ${{ always() }}
        run: |
          # fail iff the background core gate exited non-zero
          ...
      - name: Upload bounded core-gate failure evidence
        if: >-
          ${{ always() &&
              steps.core-verdict.outputs.summary_path != '' &&
              steps.core-verdict.outputs.metadata_path != '' }}
        continue-on-error: true
        uses: actions/upload-artifact@v7
        with:
          path: |
            ${{ steps.core-verdict.outputs.summary_path }}
            ${{ steps.core-verdict.outputs.metadata_path }}
          if-no-files-found: ignore
          retention-days: 7
```

The advisory ub-review lane is the separate standalone workflow of section
4.1 ("Standalone advisory ub-review lane"):

```yaml
name: UB Review

on:
  pull_request:
    types: [opened, reopened, ready_for_review, synchronize]

permissions:
  contents: read

concurrency:
  group: ub-review-${{ github.event.pull_request.number }}
  cancel-in-progress: true

jobs:
  review:
    name: UB Review (advisory)
    if: >-
      github.event.pull_request.head.repo.full_name == github.repository &&
      github.event.pull_request.draft == false
    runs-on: ubuntu-latest
    timeout-minutes: 25
    continue-on-error: true
    permissions:
      contents: read
      # Only so ub-review can post its grouped advisory PR review.
      pull-requests: write
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
          persist-credentials: false
      - uses: EffortlessMetrics/ub-review@<pinned-commit-sha>
        with:
          profile: gh-runner
          posting: review
          fail-on-gate: 'false'
          minimax-api-key: ${{ secrets.MINIMAX_API_KEY }}
          base: origin/${{ github.base_ref }}
          head: HEAD
          out: target/ub-review
      - uses: actions/upload-artifact@v7
        if: always()
        with:
          name: ub-review-artifacts
          path: target/ub-review
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
      - name: Write bounded GitHub job summary
        run: cat target/unsafe-review/github-summary.md >> "$GITHUB_STEP_SUMMARY"
      - uses: actions/upload-artifact@v7
        if: always()
        with:
          name: unsafe-review-first-pr
          path: |
            target/unsafe-review/review-kit.json
            target/unsafe-review/cards.json
            target/unsafe-review/pr-summary.md
            target/unsafe-review/github-summary.md
            target/unsafe-review/cards.sarif
            target/unsafe-review/comment-plan.json
            target/unsafe-review/witness-plan.md
            target/unsafe-review/receipt-audit.md
            target/unsafe-review/receipt-audit.json
            target/unsafe-review/policy-report.json
            target/unsafe-review/policy-report.md
            target/unsafe-review/manual-candidates.json
            target/unsafe-review/manual-repair-queue.json
            target/unsafe-review/tokmd-packets.json
            target/unsafe-review/usefulness-telemetry.json
            target/unsafe-review/lsp.json
            target/unsafe-review/repair-queue.json
            target/unsafe-review/unsafe-review-gate.json
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
