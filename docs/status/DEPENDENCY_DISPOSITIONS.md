# Dependency and release PR dispositions — issue #1916

This is the live queue audit for the current #1916 release-prep slice. It is a
swarm workbench record, not a dependency freeze, scheduler, publication
authorization, or source-repository decision. Snapshot date: 2026-09-09.
Reviewed against swarm `main` at `100595121b584521117722165b5caa61cae4ca6f`
(`ra_ap_syntax 0.0.349`, `ignore 0.4.33`) with `new_source_commits=0`.

## Repository snapshot

| Repository | Base | Source-divergence posture |
| --- | --- | --- |
| `EffortlessMetrics/unsafe-review-swarm` | `100595121b584521117722165b5caa61cae4ca6f` | `new_source_commits=0`; expected unpromoted swarm work remains |
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
| `ra_ap_syntax` 0.0.349 | [#2013](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2013), merged `0154a86fce3bf384475853b20ec9403ebb6914d5` plus #2089/#2112/#2136 and #2165 (`0.0.349`) | [#551](https://github.com/EffortlessMetrics/unsafe-review/pull/551), head `40a522b57088fc9ba30722bb9632c37f1fd99db5` | Swarm `0.0.349` is integrated on the workbench at `10059512`; source #551 remains the owner-gated candidate for 0.0.341→0.0.343. New swarm #2180 (0.0.349→0.0.350, head `207177f4`) is blocked: the bump requires rustc 1.98 while `rust-toolchain.toml` pins 1.95.0, failing local targeted proof and the hosted Rust gate. Reviewed 2026-09-09. | Keep source #551 for source-owner disposition; keep swarm #2180 open behind the owner MSRV decision. No source promotion or release claim follows from the swarm merges. |
| `ignore` 0.4.33 | — | [#547](https://github.com/EffortlessMetrics/unsafe-review/pull/547), merge `fb217073fb47f1e2bd18e02a6bce774900120c74` (0.4.26→0.4.27) | Source #547 merged `ignore 0.4.27`; swarm main carries `0.4.33` at `10059512` through later merged `cargo-minor-and-patch` batches, including the `toml` 1.1.5 bump (#2179, workspace-tested). Old swarm #1874 is closed. Reviewed 2026-09-09. | Integrated; no open duplicate disposition remains. |
| GitHub Actions pins | [#2014](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2014), merged `9285040513b80279b3f2570e3ab4ede37b3ffb27`, plus paired droid-action repairs [#2183](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2183) and [#2185](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2185) | [#549](https://github.com/EffortlessMetrics/unsafe-review/pull/549) closed unmerged; new actions-group [#565](https://github.com/EffortlessMetrics/unsafe-review/pull/565), head `f5a99574` | #2183/#2185 each moved the workflow pin and the mirrored `policy/workflow-allowlist.toml` entry together with pin-sync and policy proof green; bare bumps #2181/#2184 were closed as superseded by those replacements. Source #549 is closed; source #565 (5 updates) is open and owner-gated. Reviewed 2026-09-09. | Swarm side integrated through the paired path; keep source #565 owner-gated until an equivalent paired repair lands there. |
| `signal-hook` 0.4.4 | [#1390](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1390), merge `22c37dd20b6f143fefec1ce3d232865ce0d203db` | [#515](https://github.com/EffortlessMetrics/unsafe-review/pull/515), head `c4d890a9ba016399b6fba3ce8225d07e75eb1191` | Swarm main already carries 0.4.4; source #515 remains the source-owner candidate path. | Retain source #515 for source-owner disposition; do not create a second swarm promotion. |
| RSS/self-unsafe telemetry | [#1620](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1620), closed unmerged | — | Former draft is closed; issue #1916 keeps this outside the cutline absent a separate owner decision. | Closed and parked; do not repair or merge for release optics. |

## Current disposition boundary

Reviewed 2026-09-09 at `10059512` (`ra_ap_syntax 0.0.349`, `Cargo.lock`
`0a3b6a6f12d83cc042f8798f2ad89b7ce535e27f` Git-blob SHA-256,
`new_source_commits=0`). All dispositions above were re-checked against the
current live queue, including new source actions-group #565 and the closed
states for source #549 and swarm #1620; #2135 retains its separate runner lane
and #566 was a separate dependency-policy maintenance item, now closed. The
paired repair (workflow pins and allowlist entries updated together, then
policy and Rust proof rerun) remains the recorded paired path — not a weakened
allowlist or bypassed gate. Source #565 retains the same policy shape and
remains owner-gated.

## Boundaries and next step

This audit does not freeze versions, merge source PRs, close useful open PRs,
promote swarm commits, publish crates or actions, create tags, or move public
`v1`. It remains advisory and draft. The next #1916 slice can refresh the
cutline with exact lockfile and dependency versions after the chosen candidate
path is owner-approved and the required dependency PRs have independently
passed their targeted proof.
