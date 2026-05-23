# GitHub Actions guide

This guide is for a Rust maintainer who wants to wire `unsafe-review` into
their repository's PR checks. It is the user-facing companion to
[docs/ci/PR_CI.md](PR_CI.md), which documents how this repository runs its own
CI lanes.

The design rule:

```text
Malformed or dishonest unsafe-review artifacts fail CI.
Unsafe-review findings do not fail CI by default.
```

That keeps `unsafe-review` a quiet advisory instrument, not a default blocker.
This repository enforces the full artifact contract with
`cargo run --locked -p xtask -- check-first-pr-artifacts`. The drop-in workflow
below is for downstream repositories, so it uses a portable required-file check
until a public bundle verifier is available from the installed CLI.

## Drop-in workflow

Copy [.github/examples/unsafe-review-first-pr.yml](../../.github/examples/unsafe-review-first-pr.yml)
into your repository at `.github/workflows/unsafe-review-first-pr.yml`.

The workflow renders the advisory `first-pr` bundle, checks that required
bundle files exist and are non-empty, writes a bounded GitHub job summary, and
uploads the full bundle as a workflow artifact.

```yaml
name: unsafe-review-first-pr
on:
  pull_request:
    types: [opened, reopened, synchronize, ready_for_review]
  workflow_dispatch:
permissions:
  contents: read
```

The key properties:

- `permissions: contents: read` — no write tokens, no comment posting, no
  branch mutation, no publish.
- `persist-credentials: false` — the checkout token is not left on disk.
- `cargo install unsafe-review --locked` — installs the published CLI from
  crates.io. No source checkout of unsafe-review is required in your repo.
- `unsafe-review first-pr --base "origin/${BASE_REF}"` — writes the advisory
  bundle to `target/unsafe-review/`.
- `Verify bundle file shape` — checks that each expected bundle file exists and
  is non-empty. This is not the full unsafe-review source-repo artifact
  verifier.
- `actions/upload-artifact@v7` — uploads `cards.json`, `pr-summary.md`,
  `github-summary.md`, `cards.sarif`, `comment-plan.json`, `witness-plan.md`,
  and `lsp.json` for download by reviewers.

The workflow does not:

- post comments on the PR,
- run Miri, `cargo-careful`, sanitizers, Loom, Shuttle, Kani, or Crux,
- edit source files,
- enforce a blocking policy on `unsafe-review` findings,
- request write permissions on `GITHUB_TOKEN`,
- claim memory safety, UB-free status, Miri-clean status, or site-execution
  proof.

## What reviewers see

The PR's Checks tab shows the workflow run and its job summary. The job
summary is `target/unsafe-review/github-summary.md`, a bounded fragment
produced by `unsafe-review first-pr` alongside the full bundle: header
counts plus the top card and the trust-boundary footer, without the
(potentially large) card table or witness plan. Reviewers download the
full advisory bundle from the workflow's Artifacts section.

The advisory bundle is what makes the review credible. The job summary is the
reviewer-first preview.

## CI failure semantics

The drop-in workflow fails when the CLI path or portable bundle shape fails:

- `unsafe-review` cannot install or run,
- a required file is missing,
- a required file is empty,
- artifact upload fails.

The unsafe-review source repository has a stronger verifier for trust-boundary
text, card IDs, comment-plan shape, witness-plan route limits, and saved-LSP
projection consistency. Downstream repositories should switch to that verifier
if it becomes available through the published CLI.

It does not fail when the PR has unsafe-review findings. Those are reported
through the bundle and job summary so the human reviewer can decide.

## Optional: PR-by-PR opt-in

If you do not want `unsafe-review` to run on every PR, drop the
`pull_request` trigger and keep only `workflow_dispatch`. Reviewers can then
trigger the workflow manually from the PR or branch.

```yaml
on:
  workflow_dispatch:
    inputs:
      base_ref:
        description: "Base ref to diff against (e.g. main)"
        required: false
        default: ""
```

## Optional: paths-ignore

For docs-only PRs you may want to skip the analyzer entirely:

```yaml
on:
  pull_request:
    paths-ignore:
      - "docs/**"
      - "**/*.md"
```

`unsafe-review` will emit zero cards for those PRs even if you do run it,
because nothing in the diff is unsafe-adjacent Rust. Skipping is purely a
runtime saver.

## What this workflow is not

- It is not a security gate. `unsafe-review` is static unsafe contract review;
  it does not prove the code free of UB, memory unsafety, or data races.
- It is not a Miri replacement. Miri runs concrete execution; `unsafe-review`
  reports missing review evidence at PR time.
- It is not a comment bot. The advisory `comment-plan.json` is in the bundle
  for future trusted poster designs; this workflow never posts.
- It is not a release gate. Release readiness, package smoke, and publication
  are separate manual lanes that this workflow does not touch.

## Next steps

- [docs/ci/PR_CI.md](PR_CI.md) — how this repository runs its own CI lanes.
- [docs/FIRST_HOUR.md](../FIRST_HOUR.md) — the maintainer first-hour
  walkthrough that pairs with this workflow.
- [docs/CLI.md](../CLI.md) — full CLI reference, including `receipt`,
  `outcome`, `repo`, and `policy report`.
