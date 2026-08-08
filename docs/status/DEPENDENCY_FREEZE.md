# Draft dependency freeze record — issue #1916

This record makes the current qualification inputs reproducible without
activating a repository-wide or permanent dependency freeze. Its machine-
readable source is
[`UNSAFE-REVIEW-DEPENDENCY-FREEZE-1916.toml`](../../plans/release-cutline/UNSAFE-REVIEW-DEPENDENCY-FREEZE-1916.toml).

Audited 2026-08-08 against swarm `main` at
`db289703c00ab9cfc1cc0ff90ff13d03506ee13c` and source `main` at
`c25d65272c760c3630eb9528b7efaae2234d9e19`. This remains a draft and is not an
active dependency freeze.

## Exact snapshot

| Repository | Commit | `Cargo.lock` SHA-256 | `ra_ap_syntax` | `ignore` | `signal-hook` |
| --- | --- | --- | --- | --- | --- |
| swarm | `db289703c00ab9cfc1cc0ff90ff13d03506ee13c` | `6dd5b53fb3ff0bcfbf2913e1c5cf0f269b95b58c2a1583d3f3fe38057318a4e7` | 0.0.343 | 0.4.31 | 0.4.4 |
| source | `c25d65272c760c3630eb9528b7efaae2234d9e19` | `aba7bae758bba26e835a01b5b6d45858b658cab1dc12b456c713cc4551203527` | 0.0.341 | 0.4.27 | 0.3.18 |

These are two live repository baselines, not one frozen release candidate.
The source candidate path remains owner-gated until its dependency PRs have
the targeted proof named in issue #1916. The swarm row is the current
workbench input, not a source-candidate or publication claim.

## Candidate inputs

- Source parser PR [#548](https://github.com/EffortlessMetrics/unsafe-review/pull/548)
  merged `ra_ap_syntax 0.0.341`; parser-specific tests, detector contracts,
  fixture/calibration parity, determinism, relevant corpus checks, and full
  `check-pr` passed before the dependency identity was synced into swarm.
- Source ignore PR [#547](https://github.com/EffortlessMetrics/unsafe-review/pull/547)
  merged `ignore 0.4.27`; directly affected core/workspace tests and the normal
  repository proof passed before the dependency identity was synced into swarm.
- Source signal-hook PR [#515](https://github.com/EffortlessMetrics/unsafe-review/pull/515)
  remains open for `signal-hook 0.4.4`; directly affected signal-handling tests
  and the normal workspace proof remain required.
- Source parser PR [#551](https://github.com/EffortlessMetrics/unsafe-review/pull/551)
  remains open for `ra_ap_syntax 0.0.343`; parser/corpus proof remains required.
- Source Actions PR [#549](https://github.com/EffortlessMetrics/unsafe-review/pull/549)
  remains owner-gated; its workflow pins and policy mirrors must move together.
- Dependency group PR [#550](https://github.com/EffortlessMetrics/unsafe-review/pull/550)
  remains owner-gated for its own affected-test and lockfile proof.

The current swarm-only queue also contains #2013 (`ra_ap_syntax 0.0.344`) and
issue #2014 (paired Actions updates). Both remain deferred from source
promotion until one history path is chosen.

The merged swarm audit [#1943](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1943)
records why swarm #1874/#1875 are deferred and why #1620 remains parked.

## Exception and reset rule

Once an owner explicitly activates this candidate-specific freeze, non-security
dependency additions are deferred through RC closeout. A security or
Rust/MSRV-critical exception must use a focused issue/PR and state its security
impact, semver impact, invalidated qualification receipts, and required
reruns. Any accepted dependency change resets the affected receipts and
requires a refreshed exact lockfile SHA before closeout.

This does not disable routine security response or Dependabot permanently. It
does not prove dependency safety, memory safety, UB-freedom, Miri cleanliness,
installed-product readiness, publication authorization, or public `v1`.
