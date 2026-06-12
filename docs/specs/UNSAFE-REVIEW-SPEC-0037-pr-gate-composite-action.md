# UNSAFE-REVIEW-SPEC-0037: PR-gate composite GitHub Action

Status: proposed
Owner: repo-infra / ci
Created: 2026-06-12

Linked specs:
- [UNSAFE-REVIEW-SPEC-0011: PR and CI output](UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md)
- [UNSAFE-REVIEW-SPEC-0024: CI design](UNSAFE-REVIEW-SPEC-0024-ci-design.md)
- [UNSAFE-REVIEW-SPEC-0034: ub-review gate manifest](UNSAFE-REVIEW-SPEC-0034-ub-review-gate-manifest.md)

Linked docs:
- [docs/ci/github-action.md](../ci/github-action.md)
- [.github/actions/unsafe-review-first-pr/action.yml](../../.github/actions/unsafe-review-first-pr/action.yml)
- [.github/examples/unsafe-review-first-pr.yml](../../.github/examples/unsafe-review-first-pr.yml)

## 1. Purpose

This spec defines the contract for the `unsafe-review-first-pr` composite
GitHub Action. The action is the primary adoption path for external
repositories that want `unsafe-review` PR coverage without copy-pasting a
workflow. It is a composite action (not Docker) that installs the published
`unsafe-review` CLI, runs `first-pr`, surfaces the advisory bundle through
step outputs, and writes a bounded GitHub job summary.

The action is advisory by design. Its defaults never fail a job on
`unsafe-review` findings, never post PR comments, never request write
permissions, and never claim memory-safety proof. See SPEC-0024 for the full
CI posture doctrine this action inherits.

Fixed advisory trust boundary (applies to all surfaces produced by or
documenting this action):

```text
Static unsafe contract review only. Not memory-safety proof, not UB-free
status, not Miri-clean status, and not site-execution proof.
```

## 2. Action placement and `uses:` line

The action lives at:

```text
.github/actions/unsafe-review-first-pr/action.yml
```

in the development repository (`unsafe-review-swarm`). Once promoted to the
source/public repository (`EffortlessMetrics/unsafe-review`), external callers
reference it as:

```yaml
uses: EffortlessMetrics/unsafe-review@v1
```

or a pinned tag/SHA equivalent. The development copy in `unsafe-review-swarm`
is not the published surface; do not reference the swarm repo from external
callers.

## 3. Binary acquisition contract

The action installs the `unsafe-review` CLI from crates.io using
`cargo install --locked --version <pin>`. A pinned version input (default
`0.3.5`) prevents silent breakage on new releases.

Binary acquisition decision: `cargo install` from crates.io is the MVP path
because the `unsafe-review` crate is published to crates.io and no
pre-compiled release binaries are currently distributed through GitHub
Releases. The `cargo install` step runs inside the caller's job and uses the
caller's Rust toolchain; the action installs `dtolnay/rust-toolchain@1.95.0`
as a prerequisite.

Optimization path: if pre-compiled binaries are later distributed as GitHub
Release assets, the action should be updated to prefer downloading a pinned
asset over `cargo install` to reduce install time. Until then, callers should
use `Swatinem/rust-cache@v2` (or equivalent) to cache the built binary across
runs.

## 4. Inputs

| Input | Required | Default | Description |
|---|---|---|---|
| `base_ref` | no | `${{ github.event.repository.default_branch }}` | Base ref to diff against (e.g. `main`) |
| `version` | no | `0.3.5` | `unsafe-review` crate version to install from crates.io |
| `out_dir` | no | `target/unsafe-review` | Directory for the advisory bundle output |
| `fail_on_new_debt` | no | `false` | When `true`, exit non-zero if new or worsened coverage gaps are found (maps to exit 1; inherited gaps never fail). Advisory by default — callers must set this explicitly to change the default. |

`fail_on_new_debt: true` maps to the existing `--policy no-new-debt` exit
semantics (exit 1 on new or worsened gaps, diff-scoped, changed-line
attributed). Inherited gaps are never counted. With the default `false` the
action never fails the calling job on advisory findings.

## 5. Outputs

| Output | Description |
|---|---|
| `bundle_dir` | Absolute path to the advisory bundle directory (value of `out_dir`) |
| `gate_status` | Contents of the `status` field from `unsafe-review-gate.json` — advisory metadata only; never a merge verdict |

`gate_status` is read from `unsafe-review-gate.json` in the bundle directory.
Its value is the string from the gate manifest `status` field (e.g.
`"advisory"`). It is advisory metadata that describes coverage movement, not
a pass/fail verdict. The caller controls how, if at all, this value is used
downstream.

## 6. Artifacts produced

The action writes the following files to `out_dir` (default
`target/unsafe-review/`):

```text
review-kit.json
cards.json
pr-summary.md
github-summary.md
cards.sarif
comment-plan.json
witness-plan.md
receipt-audit.md
manual-candidates.json
manual-repair-queue.json
tokmd-packets.json
lsp.json
repair-queue.json
unsafe-review-gate.json
```

These are the same artifacts as the `first-pr` bundle defined in SPEC-0011
and SPEC-0024, plus `unsafe-review-gate.json` (SPEC-0034). The action does
NOT upload these artifacts itself — the caller controls artifact uploads and
retention. This keeps the action free of `actions/upload-artifact` calls and
allows callers to choose their own upload strategy.

## 7. Advisory posture and permissions

The action does NOT:

- post PR comments
- run Miri, `cargo-careful`, sanitizers, Loom, Shuttle, Kani, or Crux
- edit source files
- enforce blocking policy on `unsafe-review` findings by default
- request write permissions on `GITHUB_TOKEN`
- claim memory safety, UB-free status, Miri-clean status, or site-execution
  proof

The `action.yml` itself requests only `contents: read` through documentation;
it cannot grant permissions to a calling job. The caller owns permissions. If
the caller wants to upload SARIF to GitHub's security dashboard it must add
`security-events: write` to its own `permissions:` block. If the caller wants
to upload artifacts it must add `actions: write` or use an explicit upload
step.

The action must never receive or use write tokens for comment posting, source
editing, or branch mutation.

## 8. GitHub summary

The action appends `github-summary.md` to `$GITHUB_STEP_SUMMARY`. This
produces a bounded advisory summary in the PR's Checks tab. The summary is
produced by `unsafe-review first-pr` (SPEC-0011 section 10) and contains
header counts, the top card, and the fixed trust-boundary footer. It does not
include the full card table or witness plan.

## 9. Failure categories

The action may fail for the following reasons:

**Tool failure (exit 2):**
- `cargo install` cannot install `unsafe-review`
- Rust toolchain setup fails
- `unsafe-review first-pr` exits with code 2 (tool error / malformed input)

**Artifact failure:**
- A required bundle file is missing
- A required bundle file is empty

**Policy result (exit 1), only when `fail_on_new_debt: true`:**
- `unsafe-review first-pr` exits with code 1 (new or worsened coverage gaps
  found in diff-scoped, changed-line attributed analysis)

**Advisory pass (default):**
- `unsafe-review` found advisory cards — job passes, cards appear in bundle
- No changed unsafe-review gaps — job passes, summary says so
- `fail_on_new_debt: false` (default) — job never fails on findings

## 10. Non-goals

- Does not post comments. Comment posting belongs to a future trusted-poster
  lane (SPEC-0024 section 4.7).
- Does not block by default. Findings are advisory; the caller opts in to
  `fail_on_new_debt: true` only when deliberately setting a coverage-debt
  policy.
- Does not inherit gaps as failures. Baseline-known cards are never a
  fail-on-new-debt trigger.
- Does not request write permissions. The caller owns all permission grants.
- Does not upload artifacts. The caller controls upload strategy and
  retention.
- Does not run witnesses. Miri and other witnesses belong to targeted lanes
  (SPEC-0024 section 12).
- Does not prove memory safety, UB-free status, or Miri-clean status (unless
  a matching witness receipt is imported through the separate receipt import
  path).
- Does not act as the orchestrator. `unsafe-review` is the evidence layer;
  `ub-review` is the orchestrator that reads `unsafe-review-gate.json` and
  posts or decides.
- Is not a live end-to-end CI smoke test in this PR. A full smoke test that
  installs the binary in a GitHub Actions run and exercises it against a real
  diff is a follow-up item. The test path is: add a caller workflow in the
  `unsafe-review-swarm` repo that `uses:` the action from the local path
  `.github/actions/unsafe-review-first-pr`, triggers on `workflow_dispatch`,
  and asserts that `bundle_dir` and `gate_status` are set.

## 11. CI proof

```bash
cargo run --locked -p xtask -- check-docs
cargo run --locked -p xtask -- check-spec-status
cargo run --locked -p xtask -- check-pr
```

## 12. Lifecycle status

Proposed. Move to accepted when:

- The composite action is wired and the example caller workflow exists
- `docs/ci/github-action.md` documents the two-line adoption snippet
- `check-spec-status` passes with this spec listed
- A live end-to-end smoke test (follow-up) has run once against the published
  binary
