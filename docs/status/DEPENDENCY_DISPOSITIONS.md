# Dependency and release PR dispositions — issue #1916

This is the live queue audit for the current #1916 release-prep slice. It is a
swarm workbench record, not a dependency freeze, scheduler, publication
authorization, or source-repository decision. Snapshot date: 2026-08-09.

## Repository snapshot

| Repository | Base | Source-divergence posture |
| --- | --- | --- |
| `EffortlessMetrics/unsafe-review-swarm` | `d4ed927a06984593ec610bbb79bd19ae902dc7db` | `new_source_commits=0`; expected unpromoted swarm work remains |
| `EffortlessMetrics/unsafe-review` | `c25d65272c760c3630eb9528b7efaae2234d9e19` | acknowledged publication-sync point |

## Chosen history path

The public source repository owns publication candidates. For remaining
duplicate Dependabot updates, retain the source PR for the candidate and defer
the swarm duplicate; do not merge both copies independently. The source PRs
remain owner-gated and are not changed by this swarm audit. The already-merged
swarm #2013 is recorded as workbench state only and does not authorize source
promotion.

| Surface | Swarm PR | Source PR | Live evidence | Disposition |
| --- | --- | --- | --- | --- |
| `ra_ap_syntax` 0.0.343 → 0.0.344 | [#2013](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2013), merged `0154a86fce3bf384475853b20ec9403ebb6914d5` | [#551](https://github.com/EffortlessMetrics/unsafe-review/pull/551), head `40a522b57088fc9ba30722bb9632c37f1fd99db5` | Swarm #2013 is integrated on the workbench; source #551 remains the owner-gated source candidate for 0.0.341 → 0.0.343. | Keep source #551 for source-owner disposition; no source promotion or release claim follows from the swarm merge. |
| `ignore` 0.4.31 | — | [#547](https://github.com/EffortlessMetrics/unsafe-review/pull/547), merge `fb217073fb47f1e2bd18e02a6bce774900120c74` | Source #547 is merged, and swarm main carries 0.4.31 through a later merged dependency batch; old swarm #1874 is closed. | Integrated; no open duplicate disposition remains. |
| GitHub Actions pins | [#2014](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2014), head `190a9005ff76db32663f2f7ea17dba9985a59cf5` | [#549](https://github.com/EffortlessMetrics/unsafe-review/pull/549), head `3e54792b493199285f34b59101fef88a300f9248` | Both open PRs fail `policy-contracts`; each changes immutable action SHAs without the matching `policy/workflow-allowlist.toml` update. | Defer both until one owner-approved paired repair path updates workflows and allowlist together, then reruns policy and Rust proof. |
| `signal-hook` 0.4.4 | [#1390](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1390), merge `22c37dd20b6f143fefec1ce3d232865ce0d203db` | [#515](https://github.com/EffortlessMetrics/unsafe-review/pull/515), head `c4d890a9ba016399b6fba3ce8225d07e75eb1191` | Swarm main already carries 0.4.4; source #515 remains the source-owner candidate path. | Retain source #515 for source-owner disposition; do not create a second swarm promotion. |
| RSS/self-unsafe telemetry | [#1620](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1620), head `59b2ca6176e5b04b3045c3d5edc4c26ade5e52b0` | — | Draft, conflicted, and Rust gate failing; issue #1916 explicitly keeps this outside the cutline absent the separate owner decision. | Keep parked; do not repair or merge for release optics. |

## Current disposition boundary

This refresh makes no dependency or workflow change. In particular, it does
not repair or merge #2014: the live policy failure is the expected paired-pin
guard, not a reason to weaken the allowlist or bypass the deterministic gate.
The source Actions PR #549 has the same policy shape and remains owner-gated.

## Boundaries and next step

This audit does not freeze versions, merge source PRs, close useful open PRs,
promote swarm commits, publish crates or actions, create tags, or move public
`v1`. The next #1916 slice can refresh the cutline with exact lockfile and
dependency versions after the chosen candidate path is owner-approved and the
required dependency PRs have independently passed their targeted proof.
