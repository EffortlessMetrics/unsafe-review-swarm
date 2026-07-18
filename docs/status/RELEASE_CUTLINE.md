# Draft release cutline — issue #1915

This is a draft qualification contract, not a release, tag, source promotion,
publication, or `v1` decision. Its machine-readable source is
[`plans/release-cutline/UNSAFE-REVIEW-CUTLINE-1915.toml`](../../plans/release-cutline/UNSAFE-REVIEW-CUTLINE-1915.toml).

## Live snapshot

- Swarm base: `7aad79695d49fb5b07ab847672baa41ad2ecdd44`
- Source base: `209c76fef1da653172a21c3348d6e3a3fb1eedbd`
- Draft candidate head: `7649bff733b7f4cf1676b9b4b4fb40226c5744b5`
- `source-divergence`: `new_source_commits=0`; the swarm contains expected
  unpromoted workbench commits.
- Candidate version: not frozen. Historical `0.3.9` and `0.4.0` names are
  references only; semver follows the integrated public surface.

## Disposition

Required candidate inputs are the governance/work-spec stack (#1939 and
#1941), the canonical field map (PR #1913), recovery and current-main evidence
(#1889 and #1890), and the canonical editor/agent chain (#1907–#1910 plus the
thin-loop proof in #1887). The cutline does not claim those open issue slices
are complete; each needs its own implementation proof and hosted CI.

Deferred by default are typed repair candidates (#1911), pull diagnostics and
progress (#1912), prebuilt binaries (#1886), grouping work (#1894/#1895), and
RSS telemetry (#1620). Publication and source promotion (#1879) remain outside
this draft and require an explicit owner decision after qualification.

## Freeze rule

This draft becomes a frozen cutline only in a follow-up PR that refreshes the
SHAs, records every required issue/PR disposition, names the candidate semver,
and attaches the exact proof and installed-product matrix. A new blocker must
amend the cutline with evidence, semver impact, and schedule impact.

The release claim remains bounded: green qualification proves the listed
contracts on the named commit and environments. It does not prove memory
safety, UB-free status, Miri cleanliness, site execution, calibrated accuracy,
or publication authorization.
