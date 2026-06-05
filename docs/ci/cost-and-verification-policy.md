# Cost and verification policy

unsafe-review CI optimizes for proof per Linux-equivalent minute (LEM), not for
fewer checks. The default PR path should be deterministic, cheap, and high
signal; expensive proof is preserved by routing it to the lane where it pays.

## CI lane posture

The authoritative lane list lives in
[`policy/ci-lane-whitelist.toml`](../../policy/ci-lane-whitelist.toml), with the
design contract in
[`UNSAFE-REVIEW-SPEC-0024`](../specs/UNSAFE-REVIEW-SPEC-0024-ci-design.md).
This document groups lanes by cost and claim posture; it is not a separate lane
registry.

| Posture family | Purpose | Default PR posture |
| --- | --- | --- |
| Workspace correctness | Build, lint, test, document, and run repo policy checks | May block |
| Artifact integrity | Ensure unsafe-review artifacts are parseable, internally consistent, and honest | May block |
| Advisory evidence | Surface ReviewCards, witness routes, source-divergence, coverage, editor/review-bot signals, and posture changes for reviewer attention | Advisory by default |
| Release or trusted actions | Prove package, install, semver, publication readiness, or split-token comment posting when explicitly routed | Release/manual/deferred lanes |

Malformed or dishonest unsafe-review artifacts may fail CI. Unsafe-review
findings do not fail CI by default.

## Tool substrate

Use upstream tools as engines, but expose durable repo behavior through `xtask`,
policy ledgers, specs, and ReviewCard-derived artifacts.

| Plane | Standard engine | Repo-facing posture |
| --- | --- | --- |
| Workspace graph | `cargo_metadata`, and `guppy` when justified | `xtask` lane and package-boundary planning |
| Tests | `cargo test` today; `cargo-nextest` when introduced | Cheap deterministic PR testing; doctests remain explicit |
| Coverage | `cargo-llvm-cov` and Codecov | Advisory execution-surface telemetry |
| Source exceptions | Future `cargo-allow` wrapper plus policy ledgers | Owned exceptions, not correctness proof |
| Static mutation exposure | Future `ripr` wrapper | Advisory weak-oracle exposure signal |
| Runtime mutation | `cargo-mutants` | Targeted, nightly, or release backstop |
| Unsafe review | `unsafe-review` | Advisory ReviewCard evidence |
| UB witnesses | Miri and other witness tools | Concrete receipts only when run |
| Dependency trust | `cargo-deny`, later `cargo-vet`, RustSec / `cargo-audit` | Supply-chain policy, not unsafe-contract review |
| Release API | `cargo-semver-checks`, rustdoc JSON when needed | Release/manual proof |
| Workflows | `actionlint`, `zizmor` | Workflow health and token posture |
| Text/config | `taplo`, `typos`, markdown/link tools | Hygiene once baselined |

A workflow may call an upstream tool directly while a wrapper is being
introduced, but durable policy must name the repo-facing surface, trigger,
artifacts, cost posture, and claim boundary.

## Default vs deep validation

Default PR validation should answer whether the changed seam is reviewable and
safe to merge under the current advisory contract. It should not silently expand
into full coverage, full mutation, Miri, macOS, Windows, Docker, GPU, or release
readiness for every routine PR.

Deep validation belongs in one of these routes:

- label-selected lanes such as coverage, mutation, release, or full CI;
- main, nightly, or release workflows;
- risk packs for parser, public API, manifest, unsafe, security, or workflow
  changes;
- owner-triggered manual workflows when the PR's claim needs the extra proof.

## Branch-protection posture

Prefer one aggregate gate, such as `PR Gate Success`, over requiring every leaf
lane. The aggregate result should distinguish:

- `passed`;
- `failed`;
- `skipped-by-policy`;
- `advisory-failed`.

A skipped optional lane is a named policy decision, not proof that the lane
passed.
