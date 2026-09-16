# Installed-product qualification — issue #1921

This is PR 2 of the release qualification sequence: the human-readable record
of the installed-product execution on the exact unpublished 0.4.0 candidate.
Machine-readable execution receipt:
[`UNSAFE-REVIEW-QUALIFICATION-1921-EXECUTION.toml`](../../plans/release-cutline/UNSAFE-REVIEW-QUALIFICATION-1921-EXECUTION.toml).

Machine-readable source: [`UNSAFE-REVIEW-QUALIFICATION-1921.toml`](../../plans/release-cutline/UNSAFE-REVIEW-QUALIFICATION-1921.toml).

## Current status

| Field | Value |
| --- | --- |
| Qualification issue | [#1921](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1921) |
| Sequence | PR 2 — executed on Linux; PR 3 reruns invalidated rows after the final candidate commit |
| Candidate commit/version | source `fb955749` (`unsafe-review#568` draft) / `0.4.0` |
| Swarm cutline / source base | `785e032d` / `c25d6527` |
| Candidate lockfile SHA-256 | `91f407bdfd16abc45d26c83ee640fc72963216836d7150bbd9888a6d6ec87d36` |
| Candidate package versions | `unsafe-review-core`, `unsafe-review-cli`, `unsafe-review` — `0.4.0` |
| Installed binary SHA-256 | `e56afd30ff8c45aa4773ef01606a649a74acb33c0d9c49dbc10bc5b628e9d73a` |
| Environment | Linux x86_64, rustc 1.98.1; Windows explicitly untested |
| Installed result | pass on linux-ci; Windows rows are limitations, not passes |

## Matrix

| Lane | Required evidence | Status |
| --- | --- | --- |
| First use | Installed version, help, doctor, and shipped command discovery, including `baseline init` | pass (linux-ci) |
| Preview adoption | Deterministic JSON/human preview, conflict reporting, non-mutation | pass (linux-ci); top-level init shipped and proven |
| PR/front panel | Quiet, new, worsened, improved, inherited-only, and human-only fixture bundles | pass (linux-ci); six bundles discriminate, verifier green on all |
| Failure semantics | Complete, capped, partial, malformed, invalid-flag, and IO-failure distinctions | pass (linux-ci); genuine 1s timeout exits 2 with `completed: false` sidecars |
| Editor/agent | #1887 diagnostic → explanation → packet/route → identity → verification → refresh loop | pass (linux-ci); repo smoke plus installed-facade 9/9 session |
| Consumers | Tokmd presets, unsafe-review → ub-review ingestion, saved-consumer compatibility | pass (linux-ci); pinned tokmd 1.15.0 five presets, pinned ub-review parser ingests candidate bundle, additive field accepted |
| Proof floor | Format, clippy, workspace tests, `check-pr`, artifact verifier, cargo-allow audit, diff check | pass (linux-ci); source CI green on the exact candidate (run 35046053774) |
| Platform boundary | Named OS/architecture/toolchain and explicit skipped/unavailable limitations | pass (linux-ci); Windows named as untested, no support inferred |

The matrix is intentionally row-based. A failed or incomplete row is not a
qualified run, and a skipped row must state why. A capped scan is not a
complete scan; a partial or malformed input is not a clean no-card result.

## Receipt minimum

Every execution row records:

- candidate commit and version, source/swarm SHAs, package versions, and
  lockfile SHA-256;
- OS, architecture, toolchain, install/package method, task-owned prefix and
  target directory;
- a redacted command class, exit code, duration, bounded failure
  classification, safe execution-log reference or hash, and result (`pass`,
  `fail`, `skipped`, `not_applicable`, or `blocked`);
- full diagnostics stay in protected execution logs; committed receipts must
  not contain raw failure text, secrets, or unbounded output;
- the reason for every skip;
- the fixture/input revision and named verification command where applicable;
- trust boundary and known limitations.

Do not commit secrets, full source trees, or unbounded logs. Candidate,
package, lockfile, docs, source, or owning behavior changes invalidate affected
rows; the next execution PR must rerun them or record an explicit skip reason.

## Execution order

1. #1917 names the exact unpublished candidate and freezes its package/lockfile
   identity.
2. Install/package that candidate into clean task-owned locations; prove the
   installed binary does not resolve workspace path leakage.
3. Confirm the owner decision for additive/unknown consumer fields in #1918
   and its linked compatibility contract; do not infer a policy in the
   qualification run.
4. Run the matrix and write bounded machine/human receipts.
5. Fix candidate defects in their owning issue/PR, never inside the receipt PR.
6. Rerun invalidated rows after final docs/candidate changes before #1925.

No crates.io publication, source merge, tag, GitHub Release, public Action, or
`v1` movement is part of this matrix.

## Claim boundary

Green rows prove only the listed installed paths on the named candidate and
environments. They do not prove memory safety, UB-free status, Miri-clean
status, site execution, calibrated precision/recall, broad platform behavior,
or authorization to publish.
