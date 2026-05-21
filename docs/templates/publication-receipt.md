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
rtk cargo fmt --check
rtk cargo check --workspace --all-targets --locked
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --locked
rtk cargo run --locked -p xtask -- check-pr
rtk cargo run --locked -p xtask -- check-calibration
rtk cargo run --locked -p xtask -- check-dogfood
rtk cargo package -p unsafe-review-core --list
rtk cargo package -p unsafe-review-cli --list
rtk cargo package -p unsafe-review --list
rtk cargo publish -p unsafe-review-core --dry-run
```

Observed result:

- <observed result>

## Publish commands

Record exact commands and observed registry propagation:

```bash
rtk cargo publish -p unsafe-review-core
rtk cargo search unsafe-review-core --limit 10
rtk cargo publish -p unsafe-review-cli --dry-run
rtk cargo publish -p unsafe-review-cli
rtk cargo search unsafe-review-cli --limit 10
rtk cargo publish -p unsafe-review --dry-run
rtk cargo publish -p unsafe-review
rtk cargo search unsafe-review --limit 10
```

Observed result:

- <observed result>

## Post-publish smoke

Install the published facade from crates.io into an isolated location or with an
explicit `--force` owner decision:

```bash
rtk cargo install unsafe-review --locked --force
rtk unsafe-review --version
rtk unsafe-review doctor
rtk unsafe-review first-pr --root fixtures/raw_pointer_alignment --diff fixtures/raw_pointer_alignment/change.diff --out-dir target/unsafe-review-published-smoke
rtk cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review-published-smoke
rtk unsafe-review explain --root fixtures/raw_pointer_alignment <card-id>
rtk unsafe-review support
```

Observed result:

- <observed result>

## docs.rs checks

Check the exact version URLs:

```bash
rtk curl -I https://docs.rs/unsafe-review-core/<version>/unsafe_review_core/
rtk curl -I https://docs.rs/unsafe-review-cli/<version>/unsafe_review_cli/
rtk curl -I https://docs.rs/unsafe-review/<version>/unsafe_review/
```

Observed result:

- <observed result>

## Tag follow-up

Create the release tag only after the publication receipt records the verified
crate URLs, docs.rs checks, and install smoke:

```bash
rtk git tag -a v<version> <source-commit> -m "unsafe-review <version>"
rtk git push origin v<version>
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
