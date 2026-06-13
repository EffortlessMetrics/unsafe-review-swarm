# unsafe-review composite action

Add `unsafe-review` PR coverage to any Rust repository in two steps.

## Adoption

```yaml
- uses: actions/checkout@v6
  with:
    fetch-depth: 100
    persist-credentials: false
- uses: EffortlessMetrics/unsafe-review@v1
  with:
    version: "0.3.6"
```

That is the full integration. The action installs `unsafe-review` from
crates.io, runs `first-pr` against the PR base, writes a bounded advisory
summary to the GitHub job summary panel, and sets `bundle_dir` and
`gate_status` step outputs. No write tokens are needed; no comments are posted.

Full example job:

```yaml
jobs:
  unsafe-review:
    name: unsafe-review advisory packet
    runs-on: ubuntu-latest
    timeout-minutes: 30
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 100
          persist-credentials: false

      - uses: EffortlessMetrics/unsafe-review@v1
        id: ur
        with:
          version: "0.3.6"

      - uses: actions/upload-artifact@v7
        if: always()
        with:
          name: unsafe-review-first-pr
          path: ${{ steps.ur.outputs.bundle_dir }}
          if-no-files-found: error
```

The caller controls artifact uploads. The action itself does not upload
anything.

## Inputs

| Input | Default | Description |
|---|---|---|
| `base_ref` | repo default branch | Base ref to diff against |
| `version` | `0.3.6` | `unsafe-review` version from crates.io |
| `fetch_depth` | `100` | Depth passed to `git fetch --depth` when fetching the base ref. Increase for repositories with very long histories. |
| `out_dir` | `target/unsafe-review` | Bundle output directory |
| `fail_on_new_debt` | `false` | When `true`, fail the job on new or worsened coverage gaps (never on inherited gaps). Advisory by default. |

## Outputs

| Output | Description |
|---|---|
| `bundle_dir` | Absolute path to the advisory bundle directory |
| `gate_status` | Advisory status string from `unsafe-review-gate.json` — coverage metadata, not a merge verdict |

## What appears in the bundle

The bundle at `bundle_dir` contains:

```text
review-kit.json            — review handoff packet with bounded card queue
cards.json                 — ReviewCard list
pr-summary.md              — maintainer cockpit view
github-summary.md          — bounded job-summary fragment (already written to $GITHUB_STEP_SUMMARY)
cards.sarif                — SARIF projection for GitHub code scanning
comment-plan.json          — plan-only comment budget (not posted)
witness-plan.md            — external witness routes per card
receipt-audit.md           — saved receipt metadata summary
manual-candidates.json     — manual review candidates
manual-repair-queue.json   — manual repair queue sidecar
tokmd-packets.json         — formatting input sidecar
usefulness-telemetry.json  — operational diagnostic telemetry (SPEC-0038)
lsp.json                   — saved LSP projection
repair-queue.json          — repair queue with bucket reasons
unsafe-review-gate.json    — advisory gate manifest (SPEC-0034)
```

## What the bundle means

- The bundle is advisory coverage evidence: it tells reviewers which unsafe
  changes lack a safety contract, guard, test, or witness.
- `cards.json` contains ReviewCards, one per detected unsafe seam with missing
  evidence.
- `pr-summary.md` is the maintainer cockpit: open it to see the top card,
  operation, missing evidence, and next action.
- `comment-plan.json` is a plan; `unsafe-review` does not post it.
- `unsafe-review-gate.json` is a routing manifest for orchestrators such as
  `ub-review`. Its `gate_status` output is advisory metadata, never a merge
  verdict.
- `witness-plan.md` lists external tool routes (Miri, cargo-careful,
  sanitizers, Loom). These are suggestions; `unsafe-review` does not run them.

## What the bundle does NOT mean

- It does not prove the code free of undefined behavior.
- It does not certify the PR as memory-safe or UB-free.
- It is not a Miri result or a sanitizer result.
- It does not prove any unsafe site was reached by a test.
- It does not block the merge by default (set `fail_on_new_debt: true` to
  opt in to a no-new-debt policy, which fails only on new or worsened gaps in
  the diff, never on inherited gaps).

Trust boundary: Static unsafe contract review only. Not memory-safety proof,
not UB-free status, not Miri-clean status, and not site-execution proof.

## CI failure semantics

The action fails when:

- `cargo install unsafe-review` cannot complete (tool error),
- `unsafe-review first-pr` exits with code 2 (internal tool error),
- a required bundle file is missing or empty,
- `fail_on_new_debt: true` and exit code 1 (new or worsened gaps in the diff).

The action does NOT fail when unsafe-review finds advisory cards (default
behavior).

## Binary acquisition

The action installs `unsafe-review` from crates.io via
`cargo install unsafe-review --locked --version <version>`. The `version`
input pins the installed release. Use `Swatinem/rust-cache@v2` to cache the
built binary across runs and reduce install time.

If pre-compiled binaries become available as GitHub Release assets in a future
release, the action will prefer downloading a pinned asset over
`cargo install`. Until then, the `cargo install` path is the only supported
mechanism.

## Permissions

The action needs no write tokens. The minimum caller permissions block is:

```yaml
permissions:
  contents: read
```

To upload SARIF to GitHub's security dashboard, add `security-events: write`.
To upload the bundle as a workflow artifact, add `actions: write` (or use
`actions/upload-artifact` in the same job).

## Published action vs. development copy

The published action lives in `EffortlessMetrics/unsafe-review` and is
referenced as `uses: EffortlessMetrics/unsafe-review@v1`. The development
copy in `unsafe-review-swarm` at
`.github/actions/unsafe-review-first-pr/action.yml` is not the published
surface; do not reference the swarm repository from external callers.

## Spec reference

[UNSAFE-REVIEW-SPEC-0037](../specs/UNSAFE-REVIEW-SPEC-0037-pr-gate-composite-action.md)
defines the full contract: inputs, outputs, artifacts, advisory posture,
failure categories, and non-goals.
