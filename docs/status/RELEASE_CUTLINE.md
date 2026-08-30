# Draft release cutline — issue #1915

This is a draft qualification contract, not a release, tag, source promotion,
publication, or `v1` decision. Its machine-readable source is
[`plans/release-cutline/UNSAFE-REVIEW-CUTLINE-1915.toml`](../../plans/release-cutline/UNSAFE-REVIEW-CUTLINE-1915.toml).

## Live snapshot

Audited 2026-08-29.

- Swarm base: `ed345b71f84e3cc4344fba52c21322f8fd4efbf6`
  (`refactor(xtask): extract check dispatch (#1806) (#2150)`)
- Source base: `c25d65272c760c3630eb9528b7efaae2234d9e19`
  (`sync: remove residual RTK command guidance (#559)`)
- Draft candidate head: `ed345b71f84e3cc4344fba52c21322f8fd4efbf6` (refresh candidate; advisory only, not a freeze, tag, or publication).
- `source-divergence`: `new_source_commits=0`, `raw_swarm_only=309`; the swarm
  contains expected unpromoted workbench commits. See `cargo run --locked -p xtask -- source-divergence`.
- Candidate version: not frozen. All three published crates are still `0.3.8`.
  Historical `0.3.9` and `0.4.0` names are references only; semver follows the
  integrated public surface.

The previous drafts named `7649bff733b7f4cf1676b9b4b4fb40226c5744b5` (no object on any swarm or source ref, withdrawn) and `aae31001431a69f4a4fc318423d1566257eebde1` (mio pilot #2101). No qualification result was recorded against the withdrawn SHA. This refresh moves the draft candidate to `ed345b71` to reflect the integrated slices `b8ddd802..ed345b71` (PRs #2141–#2150); it remains an advisory snapshot, not a qualification, safety, or publication claim.

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
| Action-first PR front door (#1884) | partial — changed-file scope, movement, comment-plan, selected-action, missing-base recovery, brownfield baseline, compact artifact handoff, minimal-repro cue, compact overview-count, deduplicated top-card action, compact manual-candidate terminal handoff, duplicate manual-queue-summary removal, copyable external-PR setup routes, compact secondary handoffs, and the bounded `pr` front panel are integrated; broader one-screen front-panel compression remains open |
| Guided preview-only init (#1885) | on swarm main — top-level preview-only proposal, conflict/diff/rollback guidance, deterministic JSON, and release-placeholder boundary are integrated; adoption remains non-mutating and unpublished |
| Recovery audit (#1889) | on swarm main — #2015 |
| Current-main false-actionability evidence (#1890) | on swarm main — #2017; source #541 dispositioned and closed; the 13-card result was re-confirmed on `79e83eef` after the `ra_ap_syntax` 0.0.344 bump |
| Tokmd packet acceptance (#1857) | on swarm main — #2016 plus the fresh current-main five-preset consumer receipt (#2083, tokmd 1.15.0 from crates.io) |
| Thin editor/agent loop proof (#1887) | on swarm main — scripted parity, freshness, quietness, partial/failure, coalescing, and live protocol rails; installed qualification and usability study remain open |
| Shell-safe first-pr roots (PR #2095) | open draft — required Linux Rust result is failing; retained evidence reports only `test=101`, so failing-test identity is not proven; #2100 stopped before implementation because stable libtest output is spoofable and the alternative substrate is owner-gated |

Integration on swarm main is not a qualification result. Each item still needs
its own implementation proof and hosted CI before freeze.

### Done / closed since 2026-08-14 (b8ddd802..ed345b71) — advisory

| Item | Disposition | Integration | Evidence |
|---|---|---|---|
| Human CLI ReviewCard field parity (#2114) | done (was builder_ready) | on swarm main | `27d1563f` in #2141 |
| Schema identity and consumer compatibility (#2115) | done (was builder_ready) | on swarm main | `27d1563f` in #2141 |
| Baseline-add ReviewCard snapshot parity (#2122) | done (was builder_ready) | on swarm main | `f7bbb06b` in #2142 |
| Mutation-directive vs reference-noun governance (#2110) | done (was builder_ready) | on swarm main | `6f595070` in #2143 |
| Ub-review artifact smoke (#2116) | done (was builder_ready) | on swarm main | `bdbfef68` in #2144, `docs/receipts/ub-review-smoke-6f595070.md` |
| Shared rowless ReviewCard details (#2124) | done | on swarm main | `e266d490` in #2145 |
| Rowless agent-packet details (#2125) | done | on swarm main | `c5a8c496` in #2146 |
| Rowless witness-plan details (#2126) | done | on swarm main | `8d91fae3` in #2147 |
| Outcome snapshot projection parity (#2127) | done | on swarm main | `48b082b7` in #2148 |
| First-PR terminal ReviewCard parity (#2121) | done | on swarm main | `96766340` in #2149 |
| Xtask check-dispatch extraction (#1806 slice) | required, partial | partial on swarm main | `ed345b71` in #2150; lane remains open |
| CI cost / hosted fallback gate (#1515) | closed | on swarm main | `e3912c11` (#2085), `20905d31`, `d873b8e7`; closed `2026-08-29T11:56:45Z` as COMPLETED |

All rows are advisory; none claims memory safety, UB-free, Miri-clean, or calibrated precision/recall. `done` records GitHub closed state plus on-swarm-main integration, not a safety or qualification claim.

### Deferred inputs

| Item | Integration |
|---|---|
| Typed repair candidates (#1911) | on swarm main |
| Grouping work (#1894, #1895) | on swarm main |
| Pull diagnostics and progress (#1912) | not on swarm main |
| Prebuilt binaries (#1886) | not on swarm main |
| RSS telemetry (PR #1620) | parked open draft at `59b2ca61`, conflicted with a failing required Rust result; the self-unsafe product posture remains owner-gated |
| External-pilot usefulness (#1881) | partial on swarm main — ten exact development-binary receipts cover quiet, inherited-only, new-gap, and resolved/improved cases, but the rollup records `public_action=false`; no public Action or released-binary claim |
| Hostile-input and fail-closed matrix (#1883) | partial on swarm main — deterministic slices through #2061 are integrated; unreadable-file coverage and the broader process-control/resource matrix await an owner-approved test strategy |

Typed repair candidates and grouping work were deferred as not required to
establish the candidate contract, and both have since landed additively on
swarm main. They therefore form part of the candidate surface that qualification
must cover, even though they do not gate the cutline. Deferral records only that
an item does not gate the cutline — never that it is absent from the candidate.

PR #1620 remains parked because its self-unsafe posture is an owner-gated
product decision. #1883's platform-sensitive remainder likewise needs an owner
decision on a reliable test strategy. Neither is promoted to release work by
this refresh.

The ten #1881 receipts are development-binary usefulness evidence. They do not
satisfy the issue's public-Action criterion: the rollup remains
`public_action=false`. Public Action execution and released-binary evidence are
not inferred, and aggregate movement remains a judgment input rather than an
accuracy or improvement claim.

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
