# Installed-product qualification — issue #1921

This is PR 1 of the release qualification sequence: a human-readable matrix
and receipt contract. It defines the work that a later execution PR must run
on one exact unpublished candidate. It is not an execution receipt.

Machine-readable source: [`UNSAFE-REVIEW-QUALIFICATION-1921.toml`](../../plans/release-cutline/UNSAFE-REVIEW-QUALIFICATION-1921.toml).

## Current status

| Field | Manifest value |
| --- | --- |
| Qualification issue | [#1921](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1921) |
| Sequence | PR 1 — manifest only |
| Candidate commit/version | unset; #1917 must produce the candidate |
| Current swarm baseline | `94b5cd4a8d2200c9ba1abc666886eea5d51dcf5c` |
| Current source baseline | `c25d65272c760c3630eb9528b7efaae2234d9e19` |
| Baseline lockfile SHA-256 | `6dd5b53fb3ff0bcfbf2913e1c5cf0f269b95b58c2a1583d3f3fe38057318a4e7` |
| Baseline package versions | `unsafe-review-core`, `unsafe-review`, `unsafe-review-cli` — `0.3.8` |
| Installed result | not run |

The baseline values make the manifest reviewable today. They do not substitute
for the candidate identity: the execution receipt must replace them or carry
them as parent-baseline fields alongside the exact candidate values.

## Matrix

| Lane | Required evidence | Status |
| --- | --- | --- |
| First use | Installed version, help, doctor, and shipped command discovery, including `baseline init` | not run |
| Preview adoption | Deterministic JSON/human preview, conflict reporting, non-mutation | not run; only if top-level init is shipped |
| PR/front panel | Quiet, new, worsened, improved, inherited-only, and human-only fixture bundles | not run |
| Failure semantics | Complete, capped, partial, malformed, invalid-flag, and IO-failure distinctions | not run |
| Editor/agent | #1887 diagnostic → explanation → packet/route → identity → verification → refresh loop | not run |
| Consumers | Tokmd presets, unsafe-review → ub-review ingestion, saved-consumer compatibility | not run |
| Proof floor | Format, clippy, workspace tests, `check-pr`, artifact verifier, cargo-allow audit, diff check | not run |
| Platform boundary | Named OS/architecture/toolchain and explicit skipped/unavailable limitations | not run |

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
