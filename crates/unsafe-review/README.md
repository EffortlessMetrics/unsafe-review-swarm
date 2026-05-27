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

`unsafe-review` is the CLI install handle for advisory unsafe Rust PR review.
It turns changed unsafe-adjacent code into ReviewCards, a PR summary, witness
routes, and read-only projection artifacts that a maintainer can inspect before
asking for a guard, test, contract, or receipt.

It is not a UB prover, Miri replacement, policy gate, or automatic reviewer.

## Install

```bash
cargo install unsafe-review --locked
unsafe-review doctor
```

Most users should install this `unsafe-review` crate.
Use [`unsafe-review-core`](https://crates.io/crates/unsafe-review-core) only
when embedding the analysis engine programmatically.

## First PR Review

Run the first-use PR cockpit from a repository checkout:

```bash
unsafe-review doctor
unsafe-review first-pr --base origin/main
```

Then open the summary and inspect the top card:

```bash
open target/unsafe-review/pr-summary.md
unsafe-review explain <card-id>
unsafe-review support
```

The intended loop is:

```text
changed unsafe seam
-> ReviewCard
-> missing evidence
-> concrete reviewer action
-> optional witness route or receipt
```

## Artifact Bundle

`first-pr` writes a standard advisory bundle under `target/unsafe-review/`:

```text
cards.json
pr-summary.md
github-summary.md
cards.sarif
comment-plan.json
witness-plan.md
lsp.json
```

The bundle is artifact-first:

| Artifact | Use |
|---|---|
| `cards.json` | Canonical ReviewCard data |
| `pr-summary.md` | Reviewer first screen |
| `github-summary.md` | Bounded GitHub job summary text |
| `cards.sarif` | Code scanning / CI artifact |
| `comment-plan.json` | Planned comments, not posted |
| `witness-plan.md` | Suggested witness routes and limits |
| `lsp.json` | Saved read-only editor projection |

## Explain One Card

Use `explain` when a card needs reviewer context:

```bash
unsafe-review explain <card-id>
```

The explanation should answer:

```text
why this card exists
which safety conditions matter
which evidence was found
which evidence is missing
what would resolve it
what would not resolve it
which witness route is credible
what unsafe-review is not claiming
```

## Trust boundary

`unsafe-review` reports static review evidence. It is not a proof of memory safety,
not a UB-free claim, not a Miri result, not soundness evidence, and not evidence
that any unsafe site executed.

By default it does not run witnesses, post comments, edit source, enforce
blocking policy, or claim calibrated precision or recall.

Findings are advisory unless you explicitly build a separate policy around the
artifacts.

## Common Commands

| Need | Command |
|---|---|
| Check first-run readiness | `unsafe-review doctor` |
| Build a PR review bundle | `unsafe-review first-pr --base origin/main` |
| Explain a card | `unsafe-review explain <card-id>` |
| Show support posture | `unsafe-review support` |
| Print a bounded agent packet | `unsafe-review context <card-id> --json` |
| Inspect repo posture | `unsafe-review repo --format markdown` |
| Generate badge endpoints | `unsafe-review badges --out badges/` |
| Audit saved witness receipts | `unsafe-review receipt audit` |
| Compare saved snapshots | `unsafe-review outcome --before before.json --after after.json` |

## Links

| Topic | Link |
|---|---|
| Repository | [EffortlessMetrics/unsafe-review](https://github.com/EffortlessMetrics/unsafe-review) |
| First-use guide | [docs/FIRST_USE.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/FIRST_USE.md) |
| CLI guide | [docs/CLI.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/CLI.md) |
| Support summary | [docs/status/SUPPORT_SUMMARY.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/status/SUPPORT_SUMMARY.md) |
| Support tiers | [docs/status/SUPPORT_TIERS.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/status/SUPPORT_TIERS.md) |
| ReviewCard trust boundary | [docs/explanation/review-cards-and-trust-boundary.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/explanation/review-cards-and-trust-boundary.md) |
| Explain examples | [docs/explanation/explain-examples.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/explanation/explain-examples.md) |
| Dogfood evidence | [docs/dogfood/index.md](https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/dogfood/index.md) |

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT license
