<p align="center">
  <img src="unsafe-review-logo.svg" alt="unsafe-review warning mark" width="120" />
</p>

<h1 align="center">unsafe-review</h1>

<p align="center">
  <em>Advisory unsafe-contract review for Rust PRs.</em>
</p>

<p align="center">
  <a href="https://github.com/EffortlessMetrics/unsafe-review/actions/workflows/ci.yml"><img src="https://github.com/EffortlessMetrics/unsafe-review/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI" /></a>
  <a href="https://docs.rs/unsafe-review"><img src="https://docs.rs/unsafe-review/badge.svg" alt="docs.rs" /></a>
  <a href="https://crates.io/crates/unsafe-review"><img src="https://img.shields.io/crates/d/unsafe-review.svg?label=crates.io%20downloads" alt="crates.io downloads" /></a>
  <a href="https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field"><img src="https://img.shields.io/badge/MSRV-1.95-blue.svg" alt="MSRV" /></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License: MIT OR Apache-2.0" /></a>
</p>

`unsafe-review` points reviewers and coding agents at changed Rust `unsafe`
seams that are missing review evidence: a safety contract, local guard, test
reach, or witness receipt.

It does **not** prove unsafe Rust sound.

It makes unsafe Rust reviewable.

The first useful run should feel small:

```text
one PR
-> one changed unsafe contract
-> one missing evidence gap
-> one recommended guard, test, or witness route
```

Miri asks:

```text
Did this concrete execution hit UB?
```

`unsafe-review` asks the cheaper PR-time question:

```text
Does this unsafe change have the safety contract, guard, test reach,
and witness route needed to make review credible?
```

## Trust boundary

`unsafe-review` reports static review evidence.

It is **not** a proof of memory safety, **not** a UB-free claim, and **not** a
Miri result unless a matching witness receipt is attached.

It is advisory by default:

* no witness execution
* no automatic comments
* no source edits
* no default blocking policy

## Install

```bash
cargo install unsafe-review --locked
unsafe-review --version
unsafe-review doctor
```

Most users should install the `unsafe-review` façade crate.

Programmatic users should depend on
[`unsafe-review-core`](https://crates.io/crates/unsafe-review-core).

## Quick start

Review the current PR against `main`:

```bash
unsafe-review first-pr --base origin/main
```

Explain one finding:

```bash
unsafe-review explain <card-id>
```

`first-pr` writes advisory review artifacts under `target/unsafe-review/`:

```text
cards.json
pr-summary.md
cards.sarif
comment-plan.json
witness-plan.md
```

It does not run witnesses, post comments, edit source, or enforce blocking
policy.

Try the bundled smoke fixture:

```bash
unsafe-review first-pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff
```

For routine PR checks, you can also use:

```bash
unsafe-review check --base origin/main
```

## What unsafe-review produces

| Surface              | Output                       | Use                                     |
| -------------------- | ---------------------------- | --------------------------------------- |
| Review cards         | JSON / human / Markdown      | Canonical evidence object               |
| PR summary           | `pr-summary.md`              | Reviewer first screen                   |
| SARIF                | `cards.sarif`                | Code scanning / CI artifact             |
| Comment plan         | `comment-plan.json`          | Proposed comments, not posted           |
| Saved LSP projection | `lsp.json`                   | Read-only editor diagnostics and hovers |
| Agent packet         | `context <card-id> --json`   | Bounded LLM repair context              |
| Receipt audit        | JSON / Markdown              | Match saved witness receipts to cards   |
| Outcome comparison   | JSON / Markdown              | Compare saved snapshots                 |
| Repo posture         | JSON / Markdown / badge JSON | Count open review gaps, not safety      |
| Policy report        | JSON / Markdown              | Advisory no-new-debt simulation         |

## Choose a path

| If you need to...            | Start with...                                                   | Typical output                                    |
| ---------------------------- | --------------------------------------------------------------- | ------------------------------------------------- |
| Review a PR                  | `unsafe-review check --base origin/main`                        | ReviewCards and PR summary                        |
| First-use PR review          | `unsafe-review first-pr --base origin/main`                     | Cards, summary, SARIF, comment plan, witness plan |
| Feed CI artifacts            | `--format json`, `--format sarif`, `--format pr-summary`        | Uploaded advisory artifacts                       |
| Explain one finding          | `unsafe-review explain <card-id>`                               | Human-readable contract gap                       |
| Inspect support posture      | `unsafe-review support`                                         | Experimental / advisory / deferred boundaries     |
| Hand work to an agent        | `unsafe-review context <card-id> --json`                        | Bounded repair packet                             |
| Audit saved witness receipts | `unsafe-review receipt audit`                                   | Matched / stale / duplicate receipt report        |
| Compare before/after posture | `unsafe-review outcome --before before.json --after after.json` | New / resolved / improved / regressed cards       |
| Inspect repo posture         | `unsafe-review repo --format markdown`                          | Open unsafe-review gaps                           |
| Simulate policy              | `unsafe-review policy report`                                   | Advisory no-new-debt report                       |

## What works today

* **Fixture-backed ReviewCards** for many core unsafe operation families.
* **Dogfood-backed evidence rules** across selected real Rust crates and PR diffs.
* **Advisory PR artifacts**: cards JSON, PR summary, SARIF, and comment plan.
* **Read-only projections** for saved LSP/editor output and bounded agent packets.
* **Saved receipt audit** for imported witness metadata.
* **Outcome and repo posture reports** for before/after movement and open gaps.
* **Advisory policy reports** for no-new-debt simulation.

Everything above remains experimental and advisory.

No current surface is calibrated as a blocking policy gate.

## Crate surface

```text
unsafe-review          # product façade / install handle
unsafe-review-cli      # CLI adapter and rendering
unsafe-review-core     # SDK / analysis engine
```

The crate boundary policy is: design seams like microcrates, implement most as
module families, and publish only seams that deserve a support promise.

## Status and docs

This crate page is the front door, not the support ledger. Current proof and
support posture live in the status docs.

| Area                      | Link                                                                                                                                                                    |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Repository                | [EffortlessMetrics/unsafe-review](https://github.com/EffortlessMetrics/unsafe-review)                                                                                 |
| First-use guide           | [docs/FIRST_USE.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/FIRST_USE.md)                                                                   |
| CLI guide                 | [docs/CLI.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/CLI.md)                                                                               |
| Support summary           | [docs/status/SUPPORT_SUMMARY.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/status/SUPPORT_SUMMARY.md)                                         |
| Support tiers             | [docs/status/SUPPORT_TIERS.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/status/SUPPORT_TIERS.md)                                             |
| ReviewCard trust boundary | [docs/explanation/review-cards-and-trust-boundary.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/explanation/review-cards-and-trust-boundary.md) |
| Reviewer examples         | [docs/explanation/explain-examples.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/explanation/explain-examples.md)                             |
| Dogfood evidence          | [docs/dogfood/index.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/dogfood/index.md)                                                           |

## License

Licensed under either of:

* Apache License, Version 2.0
* MIT license
