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
unsafe-review-core 0.3.1 -> 0.3.2
unsafe-review-cli  0.3.1 -> 0.3.2
unsafe-review      0.3.1 -> 0.3.2
```

Facade-only patching is allowed only when the release-prep PR documents why the
facade crate is the only published surface that changed.

For the current repo-dogfood case, `0.3.2` is a pre-1.0 repo usability patch
after a history-preserving swarm import. It is not memory-safety proof, UB-free
status, Miri-clean status, site-execution proof, calibrated precision/recall,
policy readiness, witness execution, automatic comment release, or
source-editing release.

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

## Publish

Publish dependency crates first when bumping all public crates:

```bash
rtk cargo publish -p unsafe-review-core
rtk cargo publish -p unsafe-review-cli
rtk cargo publish -p unsafe-review
```

Then smoke the installed crate from crates.io:

```bash
rtk cargo install unsafe-review --version 0.3.2 --locked --root target/install-published-0.3.2
target/install-published-0.3.2/bin/unsafe-review --version
target/install-published-0.3.2/bin/unsafe-review doctor
target/install-published-0.3.2/bin/unsafe-review repo --help
target/install-published-0.3.2/bin/unsafe-review repo --root fixtures --include '**/*.rs' --list-files
target/install-published-0.3.2/bin/unsafe-review candidate import docs/examples/manual-candidates/textdecoder-sab.json --out target/unsafe-review-published-0.3.2-candidates/R4R2-S001.json
target/install-published-0.3.2/bin/unsafe-review first-pr --root fixtures/raw_pointer_alignment --diff fixtures/raw_pointer_alignment/change.diff --out-dir target/unsafe-review-published-0.3.2-smoke
rtk cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review-published-0.3.2-smoke
target/install-published-0.3.2/bin/unsafe-review explain <card-id>
target/install-published-0.3.2/bin/unsafe-review context <card-id> --json
target/install-published-0.3.2/bin/unsafe-review support
```

Verify crates.io and docs.rs rendering for every published crate. For
package-surface hotfixes, verify the rendered package page or API content that
motivated the patch.

## Receipt and mirror

After publication:

1. Record a source publication receipt with source commit, tag, crates.io
   versions, docs.rs status, install smoke, first-pr smoke, explain/context
   smoke, support smoke, known limits, and trust boundary.
2. Mirror release metadata back to `unsafe-review-swarm`.
3. Do not import source badge counts into swarm badge endpoints unless a
   separate source-of-truth policy explicitly says to do so.

## Boundary

A crates.io patch release does not prove memory safety, UB-free status,
Miri-clean status, site execution, witness success, calibrated precision/recall,
or policy readiness. It does not execute witnesses, post comments, edit source,
or enforce blocking policy by default.
