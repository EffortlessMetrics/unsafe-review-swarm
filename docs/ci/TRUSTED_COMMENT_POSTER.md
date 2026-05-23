# Trusted comment poster design

`unsafe-review` does not post PR comments by default. The supported v0.x inline
surface is `comment-plan.json`: a verified, plan-only artifact projected from
existing `ReviewCard`s.

This document defines the future trusted poster architecture. It is a design
contract, not a live workflow.

## Purpose

A trusted poster may eventually post or update a small number of inline PR
comments from an already verified `comment-plan.json`.

The poster exists to reduce reviewer copy/paste work. It does not create new
analysis truth, run witnesses, edit source, approve PRs, or enforce policy.

## Security model

Comment posting needs `pull-requests: write`. Analyzer execution on pull
request code should not have that token.

The required architecture is split:

```text
untrusted pull_request workflow
  checkout PR code with read-only permissions
  build unsafe-review
  run unsafe-review first-pr
  run check-first-pr-artifacts
  upload cards.json, comment-plan.json, and the rest of the bundle

trusted poster workflow
  run only after the untrusted workflow completes
  download the uploaded artifact bundle
  re-verify comment-plan.json and supporting artifacts
  post or update only the validated planned comments
```

The trusted workflow must not run PR-controlled code before posting.

## Inputs

The trusted poster consumes:

```text
cards.json
comment-plan.json
pr-summary.md
witness-plan.md
lsp.json
```

`comment-plan.json` is the posting plan. `cards.json` is the identity source.
The poster may read other artifacts for audit context, but it must not infer new
findings from them.

## Reverification

Before posting, the trusted workflow must re-run the artifact verifier against
the downloaded bundle:

```bash
cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review
```

At minimum, this must prove:

- `comment-plan.json` is `mode = plan_only` and `policy = advisory`;
- every planned comment references a known `ReviewCard`;
- planned comment bodies include the posting and trust boundaries;
- there are at most three planned comments;
- planned comments do not repeat card IDs or inline anchors;
- no planned comment uses `static_unknown`, baseline-known, or suppressed class;
- no planned comment body exceeds the hard word limit;
- `not_selected` entries cannot be posted;
- no artifact claims safety, UB-free status, Miri-clean status, site execution,
  or policy authority.

If verification fails, the trusted workflow must fail closed and post nothing.

## Posting rules

The poster may post only entries from `comment-plan.json.comments[]`.

It must preserve:

- `card_id`
- `path`
- `line`
- `class`
- `operation_family`
- `next_action`
- `witness_routes`
- `verify_commands`
- `actionability`
- `trust_boundary`

It must not post:

- more than the validated plan;
- entries from `not_selected[]`;
- comments on lines not named by the plan;
- comments synthesized from `cards.json` directly;
- comments after changing the plan text.

## Idempotency

Posted comments should include a hidden marker based on stable plan identity:

```text
unsafe-review-comment:<card_id>
```

On rerun, the poster should update the existing matching comment instead of
creating a duplicate. If the card leaves `comments[]`, the poster may minimize
or resolve its previous comment, but it must not claim the unsafe seam is safe.

## Forbidden behavior

The trusted poster must not:

- rerun unsafe-review analysis;
- run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, Crux, fuzzing, or
  mutation testing;
- edit source;
- insert suppressions;
- add receipts;
- approve or reject the PR;
- turn findings into a default blocking policy;
- post a no-card "all clear" comment;
- claim safety, soundness, UB-free status, Miri-clean status, witness success,
  site execution, calibrated precision, or calibrated recall.

## Minimal trusted workflow shape

This is illustrative only. Do not copy it as a live workflow until the lane is
explicitly promoted.

```yaml
name: unsafe-review comment poster

on:
  workflow_run:
    workflows: ["unsafe-review"]
    types: [completed]

permissions:
  contents: read
  actions: read
  pull-requests: write

jobs:
  post:
    if: ${{ github.event.workflow_run.conclusion == 'success' }}
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v6
        with:
          persist-credentials: false

      - uses: dtolnay/rust-toolchain@1.95.0

      - name: Download unsafe-review artifacts
        run: |
          # Download the artifact bundle from github.event.workflow_run.
          # Do not checkout or execute untrusted PR code here.

      - name: Reverify artifacts
        run: cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review

      - name: Post planned comments
        run: |
          # Consume only target/unsafe-review/comment-plan.json comments[].
          # Update existing marker comments by card_id.
```

## Promotion checklist

Before a live poster workflow is allowed:

- SPEC-0022 and SPEC-0024 remain aligned with this document.
- `policy/ci-lane-whitelist.toml` keeps `trusted-comment-poster` deferred until
  the workflow is implemented and reviewed.
- `policy/workflow-allowlist.toml` records every action ref and permission.
- The poster implementation has tests for duplicate prevention, malformed
  artifact rejection, `not_selected` rejection, and no-card output.
- A dry-run mode shows intended comment create/update/delete operations without
  writing to GitHub.
- Hosted checks prove the trusted workflow never runs PR-controlled code with a
  write token before posting.

## Boundary

Trusted posting is still advisory PR review.

```text
Malformed artifacts fail closed.
Verified planned comments may be posted.
Unsafe-review findings do not fail CI by default.
Posted comments do not prove safety.
```
