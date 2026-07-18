# Dependency and release PR dispositions — issue #1916

This is the live queue audit for the first #1916 release-prep slice. It is a
swarm workbench record, not a dependency freeze, scheduler, publication
authorization, or source-repository decision. Snapshot date: 2026-07-18.

## Repository snapshot

| Repository | Base | Source-divergence posture |
| --- | --- | --- |
| `EffortlessMetrics/unsafe-review-swarm` | `64f1eb4caa8490de76dc5bea7469794f9ea13b9c` | `new_source_commits=0`; expected unpromoted swarm work remains |
| `EffortlessMetrics/unsafe-review` | `209c76fef1da653172a21c3348d6e3a3fb1eedbd` | acknowledged publication-sync point |

## Chosen history path

The public source repository owns publication candidates. For duplicate
Dependabot updates, retain the source PR for the candidate and defer the swarm
duplicate; do not merge both copies independently. The source PRs remain
owner-gated and are not changed by this swarm PR.

| Surface | Swarm PR | Source PR | Live evidence | Disposition |
| --- | --- | --- | --- | --- |
| `ra_ap_syntax` 0.0.341 | [#1875](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1875), head `eed5b73040304c0be905b4e8d224f62b2d3bad08` | [#548](https://github.com/EffortlessMetrics/unsafe-review/pull/548), head `e5f7ca68f5f2c8cd478bbabbc84314ab7408d68e` | Both are open/mergeable with clean current check summaries; parser acceptance still requires the targeted proof listed in issue #1916. | Defer swarm #1875; retain source #548 for the source candidate. |
| `ignore` 0.4.27 | [#1874](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1874), head `40e646427b36d0ca197e526d974e3526ed985a3a` | [#547](https://github.com/EffortlessMetrics/unsafe-review/pull/547), head `58f4b89a3cbee9ad0b4b6e6ecdb6523bf743c359` | Both are open/mergeable with clean current check summaries. | Defer swarm #1874; retain source #547 for the source candidate. |
| GitHub Actions pins | [#1906](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1906), head `ce87b0aaed34f5c3c542a0b3ebebcd883df7a932` | [#545](https://github.com/EffortlessMetrics/unsafe-review/pull/545), head `d90071ee485dc9b93869541f06586ebbe1de01df` | Both fail the policy/Rust gate because the workflow pin changes are not paired with the matching allowlist entries; #1906 specifically reports `actions/setup-node@v7` absent from the swarm allowlist, while #545 reports `ub-review@860e15e4...` absent from the source allowlist. | Repair the swarm path in this PR; leave source #545 open and owner-gated for a paired source repair. |
| `signal-hook` 0.4.4 | — | [#515](https://github.com/EffortlessMetrics/unsafe-review/pull/515), head `c4d890a9ba016399b6fba3ce8225d07e75eb1191` | Source PR is open/mergeable with a clean current check summary. | Retain source #515 for source-owner disposition; no swarm duplicate exists. |
| RSS/self-unsafe telemetry | [#1620](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1620), head `59b2ca6176e5b04b3045c3d5edc4c26ade5e52b0` | — | Draft, conflicted, and Rust gate failing; issue #1916 explicitly keeps this outside the cutline absent the separate owner decision. | Keep parked; do not repair or merge for release optics. |

## Repair in this PR

The swarm action path updates `actions/setup-node` from `v6` to `v7` in the
two editor workflows and advances the standalone advisory `ub-review` action to
commit `7abfb093a32bad67019f94b8d33bf04ec2b0621d`. The same commit updates
`policy/workflow-allowlist.toml` for all three pins. This preserves the CI
contract: the deterministic Rust gate remains the only hard blocker, while
ub-review remains advisory.

## Boundaries and next step

This audit does not freeze versions, merge source PRs, close duplicate PRs,
promote swarm commits, publish crates or actions, create tags, or move public
`v1`. The next #1916 slice can refresh the cutline with exact lockfile and
dependency versions after the chosen candidate path is owner-approved and the
required dependency PRs have independently passed their targeted proof.
