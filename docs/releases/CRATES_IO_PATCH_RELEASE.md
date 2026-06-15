# Crates.io patch release

Use this runbook for package-surface fixes after source history is safe.

## Core rule

Do not publish from a source repo that is missing the reviewed history for the
fix being published.

If the patch depends on reviewed swarm state that source does not yet contain,
repair source history first. Do not publish a crate from a source repo that is
missing the reviewed commit trail for the fix.

## Preconditions

Before preparing the patch:

- source `main` contains the relevant reviewed history,
- the fix is present in the source tree,
- no history catch-up PR is pending,
- no release tag is rewritten,
- trust-boundary wording remains intact.

For a README or image hotfix, inspect the crate README in source before
publishing.

## Versioning

Prefer synchronized public crate versions unless an explicit policy permits
facade-only patch releases.

Synchronized patch example:

```text
unsafe-review-core 0.3.2 -> 0.3.3
unsafe-review-cli  0.3.2 -> 0.3.3
unsafe-review      0.3.2 -> 0.3.3
```

Facade-only patching is allowed only when the release-prep PR documents why the
facade crate is the only published surface that changed.

For the current coverage-instrument usability case, `0.3.4` is a pre-1.0
usability patch after a history-preserving swarm import. It is not
memory-safety proof, UB-free status, Miri-clean status, site-execution proof,
calibrated precision/recall, policy readiness, witness execution, automatic
comment release, or source-editing release.

## Validation

For synchronized public crate bumps:

```bash
rtk cargo fmt --check
rtk cargo check --workspace --all-targets --locked
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --locked
rtk cargo package -p unsafe-review-core --list
rtk cargo package -p unsafe-review-cli --list
rtk cargo package -p unsafe-review --list
rtk cargo publish -p unsafe-review-core --dry-run
rtk cargo publish -p unsafe-review-cli --dry-run
rtk cargo publish -p unsafe-review --dry-run
```

For facade-only patches:

```bash
rtk cargo fmt --check
rtk cargo check --workspace --all-targets --locked
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --locked
rtk cargo package -p unsafe-review --list
rtk cargo publish -p unsafe-review --dry-run
```

For README/image hotfixes, inspect packaged README content before publishing.

## Publishing via the crates-publish action

The primary publish path is the `crates-publish` GitHub Actions workflow
(`.github/workflows/crates-publish.yml`), dispatched manually from the Actions
tab on the **source/publish repo** (`EffortlessMetrics/unsafe-review`) — not the
swarm workbench. Publishing runs from source because that is the release repo and
because the workspace version bump lives on source `main` (the workflow's
version-match guard only passes where the crates are at the release version).
This workflow is owner-driven, `workflow_dispatch`-only, and must be dispatched
from `main`. It requires the `CARGO_REGISTRY_TOKEN` org secret (EffortlessMetrics
org-level, selected-repositories scope).

**One-time prerequisite (org admin):** the `CARGO_REGISTRY_TOKEN` org secret must
list `EffortlessMetrics/unsafe-review` under its selected repositories
(Org → Settings → Secrets and variables → Actions → `CARGO_REGISTRY_TOKEN` →
Repository access). Until that grant exists the workflow reaches its empty-secret
guard and fails fast; this is why historical releases used the manual fallback
below.

Typical sequence:

1. Dispatch `crates-publish` with `dry_run=true` and the target version (e.g.
   `0.3.7`). The workflow verifies that all three crate versions match each
   other and the input, then runs `cargo publish --dry-run` for core and
   `cargo package --list` for cli and the facade. Review the step summary and
   logs to confirm the file manifests are correct.
2. Once the dry run is clean, dispatch again with `dry_run=false`. The workflow
   publishes in dependency order (core first, then cli, then facade) with a
   sparse-index settle retry loop between crates. A `## crates-publish
   completed` step summary confirms each crate and version when done.

The manual sequence below is the break-glass fallback for cases where the
workflow cannot be dispatched (e.g. network issues, secret unavailability, or
emergency hotfix from a local clone).

## Publish (break-glass manual fallback)

Publish dependency crates first when bumping all public crates:

```bash
rtk cargo publish -p unsafe-review-core
rtk cargo publish -p unsafe-review-cli
rtk cargo publish -p unsafe-review
```

Then smoke the installed crate from crates.io:

```bash
rtk cargo install unsafe-review --version 0.3.4 --locked --root target/install-published-0.3.4
target/install-published-0.3.4/bin/unsafe-review --version
target/install-published-0.3.4/bin/unsafe-review doctor
target/install-published-0.3.4/bin/unsafe-review repo --help
target/install-published-0.3.4/bin/unsafe-review repo --root fixtures --include '**/*.rs' --list-files
target/install-published-0.3.4/bin/unsafe-review repo --root fixtures/raw_pointer_alignment --format markdown --out target/unsafe-review-published-0.3.4-repo.md --timeout-seconds 300
rm -rf target/unsafe-review-published-0.3.4-fixture target/unsafe-review-published-0.3.4-smoke
cp -R fixtures/raw_pointer_alignment target/unsafe-review-published-0.3.4-fixture
mkdir -p target/unsafe-review-published-0.3.4-fixture/.unsafe-review/candidates
target/install-published-0.3.4/bin/unsafe-review candidate import docs/examples/manual-candidates/textdecoder-sab.json --out target/unsafe-review-published-0.3.4-fixture/.unsafe-review/candidates/R4R2-S001.json
target/install-published-0.3.4/bin/unsafe-review first-pr --root target/unsafe-review-published-0.3.4-fixture --diff target/unsafe-review-published-0.3.4-fixture/change.diff --out-dir target/unsafe-review-published-0.3.4-smoke
rtk cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review-published-0.3.4-smoke
target/install-published-0.3.4/bin/unsafe-review explain <card-id>
target/install-published-0.3.4/bin/unsafe-review context <card-id> --json
target/install-published-0.3.4/bin/unsafe-review support
```

Verify crates.io and docs.rs rendering for every published crate. For
package-surface hotfixes, verify the rendered package page or API content that
motivated the patch.

## Receipt and mirror

After publication:

1. Record a source publication receipt with source commit, tag, crates.io
   versions, docs.rs status, install smoke, repo help/list-files/status smoke,
   first-pr smoke, manual candidate smoke, explain/context smoke, support smoke,
   known limits, and trust boundary.
2. Mirror release metadata back to `unsafe-review-swarm` following
   [`docs/contributing/SWARM_MIRROR.md`](../contributing/SWARM_MIRROR.md)
   (squash PR, advance the `policy/source-sync.toml` checkpoint, apply the
   CHANGELOG `Unreleased` -> dated convention; do not wholesale-copy CHANGELOG).
3. Do not import source badge counts into swarm badge endpoints unless a
   separate source-of-truth policy explicitly says to do so.

## Boundary

A crates.io patch release does not prove memory safety, UB-free status,
Miri-clean status, site execution, witness success, calibrated precision/recall,
or policy readiness. It does not execute witnesses, post comments, edit source,
or enforce blocking policy by default.
