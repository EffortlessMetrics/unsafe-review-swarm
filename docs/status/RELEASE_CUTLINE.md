# Draft release cutline — issue #1915

This is a draft qualification contract, not a release, tag, source promotion,
publication, or `v1` decision. Its machine-readable source is
[`plans/release-cutline/UNSAFE-REVIEW-CUTLINE-1915.toml`](../../plans/release-cutline/UNSAFE-REVIEW-CUTLINE-1915.toml).

## Live snapshot

Audited 2026-08-08.

- Swarm base: `13f6071d0c1e7bec0f8c699c1e871556d0464ab3`
  (`ux: compact first-pr overview counts (#2064)`)
- Source base: `c25d65272c760c3630eb9528b7efaae2234d9e19`
  (`sync: remove residual RTK command guidance (#559)`)
- Draft candidate head: unset. A candidate commit is named only at freeze.
- `source-divergence`: `new_source_commits=0`, `raw_swarm_only=243`; the swarm
  contains expected unpromoted workbench commits.
- Candidate version: not frozen. All three published crates are still `0.3.8`.
  Historical `0.3.9` and `0.4.0` names are references only; semver follows the
  integrated public surface.

The previous snapshot named `7649bff733b7f4cf1676b9b4b4fb40226c5744b5` as the
draft candidate head. That SHA resolves to no object on any swarm or source ref,
so it could not be checked out, diffed, or qualified. It is withdrawn rather
than carried forward, and no qualification result was ever recorded against it.

## Disposition

The cutline tracks two independent facts per item, because conflating them is
what made the previous snapshot misleading:

- **Disposition** — is this required to qualify the candidate? An owner
  decision.
- **Integration** — is this work present on swarm main today? A fact, refreshed
  from the git log at each audit.

Deferred work can still be integrated, and required work can still be absent.

### Required inputs

| Item | Integration |
|---|---|
| Governance and work-spec stack (#1939, #1941) | on swarm main |
| Canonical field map (PR #1913) | on swarm main |
| Canonical editor/agent chain (#1907, #1909, #1910) | on swarm main |
| Editor freshness and failure states (#1908) | partial — PR1–PR4 plus the merged `xtask lsp-smoke` and cold-start repair landed; installed qualification and the separately scoped usability study remain open |
| Action-first PR front door (#1884) | partial — changed-file scope, movement, comment-plan, selected-action, missing-base recovery, brownfield baseline, compact artifact handoff, minimal-repro cue, and compact overview-count slices are integrated; broader one-screen front-panel compression remains open |
| Guided preview-only init (#1885) | on swarm main — top-level preview-only proposal, conflict/diff/rollback guidance, deterministic JSON, and release-placeholder boundary are integrated; adoption remains non-mutating and unpublished |
| Recovery audit (#1889) | on swarm main — #2015 |
| Current-main false-actionability evidence (#1890) | on swarm main — #2017; source #541 disposition posted |
| Tokmd packet acceptance (#1857) | on swarm main — #2016; named current-main consumer only |
| Thin editor/agent loop proof (#1887) | on swarm main — scripted parity, freshness, quietness, partial/failure, coalescing, and live protocol rails; installed qualification and usability study remain open |

Integration on swarm main is not a qualification result. Each item still needs
its own implementation proof and hosted CI before freeze.

### Deferred inputs

| Item | Integration |
|---|---|
| Typed repair candidates (#1911) | on swarm main |
| Grouping work (#1894, #1895) | on swarm main |
| Pull diagnostics and progress (#1912) | not on swarm main |
| Prebuilt binaries (#1886) | not on swarm main |
| RSS telemetry (PR #1620) | open draft PR, conflicted |

Typed repair candidates and grouping work were deferred as not required to
establish the candidate contract, and both have since landed additively on
swarm main. They therefore form part of the candidate surface that qualification
must cover, even though they do not gate the cutline. Deferral records only that
an item does not gate the cutline — never that it is absent from the candidate.

PR #1620 additionally proposes relaxing the workspace `unsafe_code` lint from
`forbid` to `deny`. That is a product-stance change and needs the self-unsafe
governance decision in #1805 before it can be dispositioned.

Publication and source promotion (#1879) remain outside this draft and require
an explicit owner decision after qualification.

The #1890 receipt changes the release blocker from an unclassified historical
count to one explicit production follow-up plus fixture-scope evidence. It does
not make a global accuracy or safety claim. The #1857 receipt similarly proves
only the named current-main tokmd producer/consumer path and five presets.

Where a commit does not cite its issue number, the TOML records the matching
commits under `integration_evidence` and labels them as attribution by scope and
title rather than citation. That applies to #1907, #1910, and #1911.

## Freeze rule

This draft becomes a frozen cutline only in a follow-up PR that refreshes the
SHAs, records every required issue/PR disposition, names the candidate semver,
and attaches the exact proof and installed-product matrix. A new blocker must
amend the cutline with evidence, semver impact, and schedule impact.

Because this artifact is not machine-checked by `check-pr`, its snapshot drifts
silently between audits. Re-run the audit and refresh the snapshot before
treating any value here as current.

The release claim remains bounded: green qualification proves the listed
contracts on the named commit and environments. It does not prove memory
safety, UB-free status, Miri cleanliness, site execution, calibrated accuracy,
or publication authorization.
