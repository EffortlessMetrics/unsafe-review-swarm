# First-use guide

This guide is for a maintainer trying the published `unsafe-review` CLI for the
first time.

`unsafe-review` is static unsafe contract review. It finds unsafe Rust changes
missing a safety contract, guard, test, or witness. It does not prove memory
safety, claim UB-free status, run Miri by default, post comments, or enable
blocking policy by default.

## Install

```bash
cargo install unsafe-review --locked
unsafe-review --help
```

If you are working from a local checkout, keep the installed command and the
workspace command separate. The installed command is the user path; `cargo run`
is for development.

## Get A First Card

Check local environment signals:

```bash
unsafe-review doctor
```

`doctor` checks Git/base-ref visibility, Cargo metadata readiness, artifact
directory writability, and witness-tool hints. Missing witness tools are
informational. `doctor` does not run witnesses and does not make policy
decisions.

Run against the current branch diff:

```bash
unsafe-review first-pr --base origin/main
```

This writes the standard local review bundle:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/lsp.json
```

The default policy is advisory. The bundle is artifact-only: it does not run
witness tools, post comments, edit source, or enforce blocking policy. A finding
means:

```text
This changed unsafe-adjacent seam is missing review evidence.
```

It does not mean:

```text
This code is UB.
This repository is unsafe.
Miri failed.
Miri passed.
```

If no changed unsafe-review gaps are found, the terminal, PR summary, witness
plan, and comment-plan artifact keep the same boundary: no changed gaps is not
proof that the repo is safe, UB-free, Miri-clean, or that any unsafe site
executed.

For a deterministic smoke case, run the bundled fixture from a repo checkout:

```bash
unsafe-review check \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --format json
```

That fixture should emit one `guard_missing` raw pointer alignment card.

## Write PR Artifacts Locally

For the normal first-run path, prefer the bundle command:

```bash
unsafe-review first-pr --base origin/main
```

The lower-level `check` formats remain useful when you only need one artifact.

Write the smallest reviewer-facing summary:

```bash
unsafe-review check --base origin/main \
  --format pr-summary \
  --out target/unsafe-review/pr-summary.md
```

Write SARIF for code-scanning-compatible consumers:

```bash
unsafe-review check --base origin/main \
  --format sarif \
  --out target/unsafe-review/cards.sarif
```

Plan inline review comments without posting them:

```bash
unsafe-review check --base origin/main \
  --format comment-plan \
  --out target/unsafe-review/comment-plan.json
```

All three artifacts project from the same `ReviewCard`s. The comment plan is an
artifact only; `unsafe-review` does not post comments by default.

## Inspect One Card

`first-pr` prints an `Inspect top card` command for the highest-priority card.
Run that command to see why the card exists, what evidence is missing, what would
resolve it, what would not resolve it, which witness route fits, and what
unsafe-review is not claiming:

```bash
unsafe-review explain <card-id>
```

You can also copy any other card id from JSON, human output, or the PR summary
and pass it to `explain`. For fixture-backed examples of common card families,
see [Explain examples](explanation/explain-examples.md).

Generate a bounded repair packet for an LLM or agent:

```bash
unsafe-review context <card-id> --json
```

The packet is copy-only. It includes missing evidence, allowed repairs,
do-not-do rules, verify commands, stop conditions, and the trust boundary. It
does not edit source.

## Check Repo Posture

Generate a static repo posture report:

```bash
unsafe-review repo --format markdown --out target/unsafe-review/repo-posture.md
```

Repo posture counts open unsafe-review gaps. It is not a count of raw unsafe
usage and not a safety badge.

## Work With Receipts

`unsafe-review` can import saved witness receipts, but it does not run witness
tools by default.

Audit existing receipts against current cards:

```bash
unsafe-review receipt audit \
  --base origin/main \
  --format markdown \
  --out target/unsafe-review/receipt-audit.md
```

Use this after you have run Miri, `cargo-careful`, a sanitizer, Loom, Shuttle,
Kani, or Crux outside `unsafe-review` and recorded a receipt.

## Compare Two Snapshots

When you have two saved JSON outputs:

```bash
unsafe-review outcome \
  --before target/unsafe-review/before.json \
  --after target/unsafe-review/after.json \
  --format markdown \
  --out target/unsafe-review/outcome.md
```

Outcome comparison reads existing snapshots. It does not rerun analysis, run
witnesses, or make a policy decision.

## Next Step

For command details, receipt import examples, policy report examples, and output
format reference, see the [CLI guide](CLI.md).
