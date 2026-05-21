<p align="center">
  <img src="https://raw.githubusercontent.com/EffortlessMetrics/unsafe-review/main/unsafe-review-logo.svg" alt="unsafe-review warning mark" width="120" />
</p>

<h1 align="center">unsafe-review</h1>

<p align="center">
  <em>Advisory unsafe-contract review for Rust PRs.</em>
</p>

`unsafe-review` points reviewers and coding agents at changed Rust `unsafe` seams
that are missing review evidence: a safety contract, local guard, test reach,
or witness receipt.

It does **not** prove unsafe Rust sound. It makes unsafe Rust reviewable.

## Install

```bash
cargo install unsafe-review --locked
unsafe-review doctor
```

## Quick start

```bash
unsafe-review first-pr --base origin/main
unsafe-review explain <card-id>
```

## What you get

`unsafe-review first-pr` writes a review bundle under `target/unsafe-review/` with
`cards.json`, `report.md`, `pr-summary.md`, `comment-plan.json`, `lsp.json`,
`witness-plan.md`, and related artifacts.

## Trust boundary

`unsafe-review` reports static review evidence. It is not a proof of memory
safety, not a UB-free claim, and not a Miri result unless a matching witness
receipt is attached.

It is advisory by default: no witness execution, no automatic comments, no
source edits, and no default blocking policy.

## Programmatic use

Most users should install this façade crate. Programmatic integrations should
depend on `unsafe-review-core` directly.

## Status and support

- Root README: https://github.com/EffortlessMetrics/unsafe-review
- docs.rs: https://docs.rs/unsafe-review
- Support summary: https://github.com/EffortlessMetrics/unsafe-review/blob/main/docs/status/SUPPORT_SUMMARY.md
