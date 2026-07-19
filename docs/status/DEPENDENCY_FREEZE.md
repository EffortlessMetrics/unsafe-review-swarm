# Draft dependency freeze record — issue #1916

This record makes the current qualification inputs reproducible without
activating a repository-wide or permanent dependency freeze. Its machine-
readable source is
[`UNSAFE-REVIEW-DEPENDENCY-FREEZE-1916.toml`](../../plans/release-cutline/UNSAFE-REVIEW-DEPENDENCY-FREEZE-1916.toml).

## Exact snapshot

| Repository | Commit | `Cargo.lock` SHA-256 | `ra_ap_syntax` | `ignore` | `signal-hook` |
| --- | --- | --- | --- | --- | --- |
| swarm | `d19a87de817c3a1b4ef7dc746f25e7cc0d117eee` | `625a362746db12f1735f337ff8bf168afc721213cc139f4d20efc4c7c5b15b5a` | 0.0.341 | 0.4.27 | 0.4.4 |
| source | `4fc6eb806de1460c618d60869b8a1cb885f87eea` | `aba7bae758bba26e835a01b5b6d45858b658cab1dc12b456c713cc4551203527` | 0.0.341 | 0.4.27 | 0.3.18 |

These are two live repository baselines, not one frozen release candidate.
The source candidate path remains owner-gated until its dependency PRs have
the targeted proof named in issue #1916.

## Candidate inputs

- Source parser PR [#548](https://github.com/EffortlessMetrics/unsafe-review/pull/548)
  merged `ra_ap_syntax 0.0.341`; parser-specific tests, detector contracts,
  fixture/calibration parity, determinism, relevant corpus checks, and full
  `check-pr` passed before the dependency identity was synced into swarm.
- Source ignore PR [#547](https://github.com/EffortlessMetrics/unsafe-review/pull/547)
  merged `ignore 0.4.27`; directly affected core/workspace tests and the normal
  repository proof passed before the dependency identity was synced into swarm.
- Source signal-hook PR [#515](https://github.com/EffortlessMetrics/unsafe-review/pull/515)
  proposes `signal-hook 0.4.4`; directly affected signal-handling tests and
  the normal workspace proof remain required.
- Source Actions PR [#545](https://github.com/EffortlessMetrics/unsafe-review/pull/545)
  remains blocked until its workflow pins and allowlist mirror converge.

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
