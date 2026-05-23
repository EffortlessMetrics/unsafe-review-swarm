# Coverage telemetry (advisory)

This guide documents the advisory `coverage` CI lane and its trust
boundary. The lane runs `cargo-llvm-cov` against the workspace and uploads
the resulting LCOV to Codecov as **execution-surface telemetry only**.

## What coverage measures

`cargo-llvm-cov` reports how much of the workspace's Rust code is exercised
by `cargo test`. That is a useful signal for:

- which crates have test execution at all,
- which modules a refactor has stopped covering,
- how a new test moves the test-execution surface.

## What coverage does not measure

`unsafe-review`'s mission is unsafe-contract review evidence. Coverage does
**not** measure any of:

- memory safety,
- UB freedom,
- Miri-cleanliness,
- site-execution proof (a covered `unsafe` line is not a witness that the
  site ran with malicious inputs),
- calibrated precision or recall for `unsafe-review` itself.

Coverage is not a substitute for any unsafe-review surface. A change that
increases coverage does not retire any open `unsafe-review` gap, and a
change that decreases coverage does not introduce one.

## Workflow shape

The workflow lives at [`.github/workflows/coverage.yml`](../../.github/workflows/coverage.yml).
It is registered in `policy/workflow-allowlist.toml` (`workflow-0006`) and
in `policy/ci-lane-whitelist.toml` (`coverage` lane).

Codecov's own project / patch status checks are made **informational** via
[`codecov.yml`](../../codecov.yml). Codecov surfaces coverage telemetry, but
project / patch coverage status is informational and is not a required
branch-protection gate. Lowering coverage on a PR does not block merge by
default.

Behavior:

- triggers on `pull_request`, `push` to main, and `workflow_dispatch`,
- skips for docs-only and editor-only changes via `paths-ignore`,
- `permissions: contents: read`; no write tokens,
- runs `cargo llvm-cov --workspace --all-targets --locked --lcov`,
- uploads to Codecov with `fail_ci_if_error: false` so any Codecov outage
  does not fail the lane,
- also uploads the LCOV as a workflow artifact for offline inspection,
- never enforces a coverage threshold,
- is not a required branch-protection gate.

## README badge

The README does **not** carry a Codecov badge until the first successful
upload is observed in CI. Once it does, the badge is captioned as
execution-surface telemetry only — never as a safety claim.

## Non-goals

- No threshold gating.
- No PR comment posting on coverage delta.
- No required branch-protection or merge blocking based on coverage.
- No claim that coverage equals unsafe correctness.
- No relationship to `unsafe-review` ReviewCards, witness receipts, or
  policy reports.
