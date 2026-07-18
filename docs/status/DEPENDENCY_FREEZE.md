# Draft dependency freeze record — issue #1916

This record makes the current qualification inputs reproducible without
activating a repository-wide or permanent dependency freeze. Its machine-
readable source is
[`UNSAFE-REVIEW-DEPENDENCY-FREEZE-1916.toml`](../../plans/release-cutline/UNSAFE-REVIEW-DEPENDENCY-FREEZE-1916.toml).

## Exact snapshot

| Repository | Commit | `Cargo.lock` SHA-256 | `ra_ap_syntax` | `ignore` | `signal-hook` |
| --- | --- | --- | --- | --- | --- |
| swarm | `fb441124740681df8fbec853bce6a0f7698630cd` | `d8ba9f7081cb37ce3ca612e7f72750217f5cb79ae79d90940204f9a157283902` | 0.0.338 | 0.4.26 | 0.4.4 |
| source | `209c76fef1da653172a21c3348d6e3a3fb1eedbd` | `63edc764a500fffcc20d78f4a960aae204a04597befdbdc1cf86b3a06d6da1dd` | 0.0.336 | 0.4.26 | 0.3.18 |

These are two live repository baselines, not one frozen release candidate.
The source candidate path remains owner-gated until its dependency PRs have
the targeted proof named in issue #1916.

## Candidate inputs

- Source parser PR [#548](https://github.com/EffortlessMetrics/unsafe-review/pull/548)
  proposes `ra_ap_syntax 0.0.341`; parser-specific tests, detector contracts,
  fixture/calibration parity, determinism, relevant corpus checks, and full
  `check-pr` remain required.
- Source ignore PR [#547](https://github.com/EffortlessMetrics/unsafe-review/pull/547)
  proposes `ignore 0.4.27`; directly affected discovery/path tests and the
  normal workspace proof remain required.
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
