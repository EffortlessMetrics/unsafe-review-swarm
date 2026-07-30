# Publication Receipt: <version>

Date:
Owner:
Release PR:
Source commit:
Publication receipt PR:
Tag follow-up:

This is a publication receipt. Fill it out only after the crates, install
smoke, and docs checks have actually completed. Record the tag after it exists;
do not use this template to imply a crate, tag, docs.rs page, or smoke check
completed before it was verified.

## Published crates

Published in dependency order:

| Crate | Version | Registry URL | docs.rs |
|---|---:|---|---|
| `unsafe-review-core` | `<version>` |  |  |
| `unsafe-review-cli` | `<version>` |  |  |
| `unsafe-review` | `<version>` |  |  |

## Pre-publish verification

Record the exact source commit and commands run:

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
```

Observed result:

- <observed result>

## Publish commands

Record exact commands and observed registry propagation:

```bash
cargo publish -p unsafe-review-core
cargo search unsafe-review-core --limit 10
cargo publish -p unsafe-review-cli --dry-run
cargo publish -p unsafe-review-cli
cargo search unsafe-review-cli --limit 10
cargo publish -p unsafe-review --dry-run
cargo publish -p unsafe-review
cargo search unsafe-review --limit 10
```

Observed result:

- <observed result>

## Post-publish smoke

Install the published facade from crates.io into an isolated location or with an
explicit `--force` owner decision:

```bash
cargo install unsafe-review --locked --force
unsafe-review --version
unsafe-review doctor
unsafe-review first-pr --root fixtures/raw_pointer_alignment --diff fixtures/raw_pointer_alignment/change.diff --out-dir target/unsafe-review-published-smoke
cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review-published-smoke
unsafe-review explain --root fixtures/raw_pointer_alignment <card-id>
unsafe-review support
```

Observed result:

- <observed result>

## docs.rs checks

Check the exact version URLs:

```bash
curl -I https://docs.rs/unsafe-review-core/<version>/unsafe_review_core/
curl -I https://docs.rs/unsafe-review-cli/<version>/unsafe_review_cli/
curl -I https://docs.rs/unsafe-review/<version>/unsafe_review/
```

Observed result:

- <observed result>

## Tag follow-up

Create the release tag only after the publication receipt records the verified
crate URLs, docs.rs checks, and install smoke:

```bash
git tag -a v<version> <source-commit> -m "unsafe-review <version>"
git push origin v<version>
```

Observed result:

- <observed result>

## Trust boundary

`<version>` is an experimental static unsafe-review evidence release.

It is not:

```text
memory-safety proof
UB-free claim
Miri-clean claim
site-execution proof
target-feature availability proof
default policy gate
automatic PR comment publisher
automatic unsafe-code repair tool
```

## Known limits

- Support tiers remain experimental/advisory unless the detailed support ledger
  says otherwise.
- Real-crate dogfood is useful but not calibrated precision/recall.
- No witness tools are executed by default.
- No default no-new-debt or blocking CI policy is enabled.
- Live LSP/editor integration remains deferred unless a later release receipt
  explicitly promotes it.
- Agent packets are copy-only and do not execute repairs.

## Stop conditions and forward-fix notes

- Do not merge this receipt while any crate URL, docs.rs URL, smoke command, or
  tag status is guessed rather than observed.
- Published crates cannot be overwritten. Prefer a forward-fix release for
  publication mistakes.
- Do not yank a published crate without an explicit owner decision and a
  documented downstream-impact reason.

## Next lane

- <next lane>
