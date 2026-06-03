# UB-Risk Review CI Cookbook

This cookbook shows how to run `unsafe-review` in CI as an advisory
UB-risk review workflow.

The product line stays narrow:

```text
unsafe-review does not prove UB.
It finds unsafe seams where UB is worth investigating and tells reviewers what
evidence would make the seam reviewable.
```

Use CI to publish the review kit, not to make `unsafe-review` the PR decider.

## Default Shape

The default CI job should:

1. Install or build `unsafe-review`.
2. Run `unsafe-review first-pr --base origin/<base>`.
3. Upload the full `target/unsafe-review/` review kit.
4. Append `target/unsafe-review/github-summary.md` to
   `$GITHUB_STEP_SUMMARY`.
5. Optionally upload `cards.sarif` to Code Scanning.

The default CI job should not:

- post PR comments,
- run Miri, `cargo-careful`, sanitizers, Loom, Shuttle, Kani, or Crux,
- edit source,
- import or fabricate witness receipts,
- fail because ReviewCards exist,
- claim UB, safety, UB-free status, Miri-clean status, site execution,
  calibrated precision/recall, witness adequacy, or policy readiness.

Malformed or missing artifacts may fail CI. Advisory findings should not fail
CI by default.

If you make this workflow required in branch protection, require successful
review-kit generation and upload. Do not require zero ReviewCards.

## Copy-Ready Workflow

For most repositories, start from this artifact-only workflow. Pin
`UNSAFE_REVIEW_VERSION` to the version you have reviewed.

```yaml
name: unsafe-review UB-risk review

on:
  pull_request:
    types: [opened, reopened, synchronize, ready_for_review]
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: unsafe-review-ub-risk-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}

jobs:
  ub_risk_review:
    name: unsafe-review UB-risk review
    if: ${{ github.event_name == 'workflow_dispatch' || github.event.pull_request.draft == false }}
    runs-on: ubuntu-latest
    timeout-minutes: 30
    env:
      UNSAFE_REVIEW_VERSION: "0.3.2"
      BASE_REF: ${{ github.base_ref || github.event.repository.default_branch }}
      BUNDLE_DIR: target/unsafe-review
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 100
          persist-credentials: false

      - uses: dtolnay/rust-toolchain@1.95.0

      - name: Fetch base ref
        run: git fetch --no-tags --depth=100 origin "+refs/heads/${BASE_REF}:refs/remotes/origin/${BASE_REF}"

      - name: Install unsafe-review
        run: cargo install unsafe-review --locked --version "${UNSAFE_REVIEW_VERSION}"

      - name: Render advisory review kit
        run: |
          mkdir -p "${BUNDLE_DIR}"
          unsafe-review first-pr \
            --base "origin/${BASE_REF}" \
            --out-dir "${BUNDLE_DIR}"

      - name: Check review-kit shape
        run: |
          set -euo pipefail
          for required in \
            review-kit.json \
            cards.json \
            pr-summary.md \
            github-summary.md \
            cards.sarif \
            comment-plan.json \
            witness-plan.md \
            receipt-audit.md \
            manual-candidates.json \
            lsp.json \
            repair-queue.json
          do
            if [ ! -s "${BUNDLE_DIR}/${required}" ]; then
              echo "::error::unsafe-review review kit is missing ${required}"
              exit 1
            fi
          done

      - name: Append GitHub job summary
        run: cat "${BUNDLE_DIR}/github-summary.md" >> "${GITHUB_STEP_SUMMARY}"

      - name: Upload unsafe-review review kit
        uses: actions/upload-artifact@v7
        if: always()
        with:
          name: unsafe-review-review-kit
          path: target/unsafe-review/
          if-no-files-found: error
```

This workflow fails if `unsafe-review` cannot install or run, if the base ref
cannot be fetched, or if the expected review-kit files are missing or empty. It
does not fail merely because `cards.json` contains ReviewCards.

## Optional SARIF

SARIF upload is useful when a repository wants Code Scanning navigation for the
same advisory ReviewCards. Keep it optional because it needs a broader
permission than the artifact-only job.

If you enable SARIF upload, add this permission:

```yaml
permissions:
  contents: read
  security-events: write
```

Then add this step after the review kit is rendered:

```yaml
      - name: Upload unsafe-review SARIF
        uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: target/unsafe-review/cards.sarif
          category: unsafe-review
```

SARIF upload does not post comments, run witnesses, or make the findings
blocking by itself. If branch protection or repository rules treat Code
Scanning alerts as blocking, that is a separate repository policy decision, not
the default `unsafe-review` workflow.

## Reviewer Use

The job summary is only a doorway. Reviewers should open the uploaded review
kit and follow the normal loop:

```text
pr-summary.md
-> explain <card-id>
-> context <card-id> --json
-> witness-plan.md
-> receipt-audit.md
-> outcome comparison after repair
```

Use careful wording in PR review:

```text
A safe caller can reach this unsafe operation without satisfying its invariant.
Here is the input or state.
Here is the minimal fix shape.
Here is the regression, witness route, or receipt that would add evidence.
```

Do not write:

```text
unsafe-review found UB.
```

## Failure Semantics

Fail CI for:

- the CLI cannot be installed or executed,
- the base ref cannot be fetched,
- the review kit is missing or malformed,
- artifact upload fails,
- a repository-local verifier rejects dishonest or inconsistent artifacts.

Do not fail CI by default for:

- new ReviewCards,
- guard-missing cards,
- contract-missing cards,
- missing witness receipts,
- advisory policy-report gaps,
- SARIF results,
- comment-plan candidates.

Downstream repositories can later add explicit no-new-debt or blocking policy
after they have baselines, suppressions, calibration, and maintainer agreement.
That is not this cookbook.

## Comment Boundaries

`comment-plan.json` is included in the review kit so a reviewer can inspect
candidate inline comments. The default workflow must not post them.

Automatic comments require a separate trusted poster design with explicit
permissions, rate limits, card identity checks, and review policy. See
[Trusted comment poster](TRUSTED_COMMENT_POSTER.md) for that future lane.

## Witness Boundaries

CI may publish `witness-plan.md`, but it should not run witnesses by default.
Witness tools belong in explicit manual, targeted, nightly, release, or
repository-specific lanes.

A witness route becomes evidence only after the witness is run outside
`unsafe-review` and a current receipt is recorded and audited against the
current card identity.

## Related Docs

- [Find and fix UB-risk review seams](../FIND_AND_FIX_UB.md)
- [GitHub Actions guide](github-actions.md)
- [PR and CI model](PR_CI.md)
- [Comment-plan examples](COMMENT_PLAN_EXAMPLES.md)
- [Trusted comment poster](TRUSTED_COMMENT_POSTER.md)
