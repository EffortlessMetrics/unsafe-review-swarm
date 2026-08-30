# Dependency and release PR dispositions — issue #1916

This is the live queue audit for the current #1916 release-prep slice. It is a
swarm workbench record, not a dependency freeze, scheduler, publication
authorization, or source-repository decision. Snapshot date: 2026-08-30.
Reviewed against swarm `main` at `125de5f683286c4e8da04b76c6633a2a8e123f5a`
(`ra_ap_syntax 0.0.348`, `ignore 0.4.33`) with `new_source_commits=0`.

## Repository snapshot

| Repository | Base | Source-divergence posture |
| --- | --- | --- |
| `EffortlessMetrics/unsafe-review-swarm` | `125de5f683286c4e8da04b76c6633a2a8e123f5a` | `new_source_commits=0`; expected unpromoted swarm work remains |
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
| `ra_ap_syntax` 0.0.348 | [#2013](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2013), merged `0154a86fce3bf384475853b20ec9403ebb6914d5` plus #2089/#2112/#2136 | [#551](https://github.com/EffortlessMetrics/unsafe-review/pull/551), head `40a522b57088fc9ba30722bb9632c37f1fd99db5` | Swarm `0.0.348` (via 0.0.344→0.345→0.347→0.348) is integrated on the workbench at `125de5f6`; source #551 remains the owner-gated candidate for 0.0.341→0.0.343. Reviewed 2026-08-30. | Keep source #551 for source-owner disposition; no source promotion or release claim follows from the swarm merge. |
| `ignore` 0.4.33 | — | [#547](https://github.com/EffortlessMetrics/unsafe-review/pull/547), merge `fb217073fb47f1e2bd18e02a6bce774900120c74` | Source #547 is merged, and swarm main carries `0.4.33` at `125de5f6` through later merged `cargo-minor-and-patch` batch; old swarm #1874 is closed. Reviewed 2026-08-30. | Integrated; no open duplicate disposition remains. |
| GitHub Actions pins | [#2014](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2014), merged `9285040513b80279b3f2570e3ab4ede37b3ffb27` | [#549](https://github.com/EffortlessMetrics/unsafe-review/pull/549), head `3e54792b493199285f34b59101fef88a300f9248` | Swarm #2014 landed through the paired repair path: the workflow pin bumps and the mirrored `policy/workflow-allowlist.toml` entries moved together (reviewed head `f0e08342`), and hosted `policy-contracts` plus `Unsafe Review Rust Result` passed on the repaired head. Source #549 is unchanged and still fails the same paired-contract shape on the source side. | Swarm side integrated; keep source #549 owner-gated until an equivalent paired repair lands there. |
| `signal-hook` 0.4.4 | [#1390](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1390), merge `22c37dd20b6f143fefec1ce3d232865ce0d203db` | [#515](https://github.com/EffortlessMetrics/unsafe-review/pull/515), head `c4d890a9ba016399b6fba3ce8225d07e75eb1191` | Swarm main already carries 0.4.4; source #515 remains the source-owner candidate path. | Retain source #515 for source-owner disposition; do not create a second swarm promotion. |
| RSS/self-unsafe telemetry | [#1620](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1620), head `59b2ca6176e5b04b3045c3d5edc4c26ade5e52b0` | — | Draft, conflicted, and Rust gate failing; issue #1916 explicitly keeps this outside the cutline absent the separate owner decision. | Keep parked; do not repair or merge for release optics. |

## Current disposition boundary

Reviewed 2026-08-30 at `125de5f6` (`ra_ap_syntax 0.0.348`, `Cargo.lock`
`f40cad0ed06d2c4cedcfbaccead979186315605c5d6e1c2c6d5de986f31c03fe`,
`new_source_commits=0`). All dispositions above were re-checked against the
current live queue; no new dependency/release PR requires a changed
disposition. The #2014 paired repair (workflow pins and allowlist entries
updated together, then policy and Rust proof rerun) remains the recorded
paired path — not a weakened allowlist or bypassed gate. Source #549 retains
the same policy shape and remains owner-gated.

## Boundaries and next step

This audit does not freeze versions, merge source PRs, close useful open PRs,
promote swarm commits, publish crates or actions, create tags, or move public
`v1`. It remains advisory and draft. The next #1916 slice can refresh the
cutline with exact lockfile and dependency versions after the chosen candidate
path is owner-approved and the required dependency PRs have independently
passed their targeted proof.
