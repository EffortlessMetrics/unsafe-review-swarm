# Swarm to main promotion policy

`EffortlessMetrics/unsafe-review-swarm` is the default implementation and
dogfood proving repo.

`EffortlessMetrics/unsafe-review` is the public source-of-record repo for the
published crates, release preparation, public documentation, badge endpoints,
support posture, and curated promotion.

The split is intentional:

```text
swarm proves
source absorbs
release publishes
```

Do not treat the two repositories as equal work targets. Routine analyzer,
evidence, dogfood, candidate extraction, and experiment work starts in
`unsafe-review-swarm`. Source repo PRs should be quieter, narrower, and tied to
public record or deliberate promotion.

## Default routing

Open new work in `unsafe-review-swarm` by default when it changes:

- analyzer or evidence behavior,
- fixtures, goldens, calibration, or dogfood controls,
- artifact verifier experiments,
- policy simulation internals,
- candidate branch extraction,
- broad refactors or risky implementation cleanup.

Open work directly in `unsafe-review` only when it is one of:

- release preparation or publication receipt work,
- public README, badge, docs.rs, crate metadata, or package surface work,
- urgent source hotfix for published users,
- source-only repository hygiene,
- curated promotion from a green swarm PR or commit.

Every source repo PR must declare its route:

```text
Source: swarm PR #NN / commit SHA
Source: direct-main public or release surface
Source: direct-main urgent source hotfix
Source: direct-main source-only repo hygiene
```

If a source PR is not swarm-originated, it must say why the work belongs in
`unsafe-review` now.

## Source sync check

Before starting routine swarm development, check whether source `main` has
drifted ahead of the workbench:

```bash
cargo run --locked -p xtask -- source-divergence
```

`check-source-sync` is an alias for the same advisory report.

The report fetches `EffortlessMetrics/unsafe-review` `main` and
`EffortlessMetrics/unsafe-review-swarm` `main`, then prints:

- the fetched source and swarm heads,
- the acknowledged source sync point from `policy/source-sync.toml`,
- new source commits after that sync point,
- raw ancestry-only source/swarm divergence counts,
- swarm-only workbench commits not yet promoted to source,
- a short next-action status.

This command is advisory in its first version. It does not fail solely because
the repos diverge. If `new_source_commits` is nonzero, open a swarm sync PR
before routine implementation work unless the source-only commits are already
accounted for by an in-flight sync or source-public-surface exception. Raw
ancestry divergence can remain nonzero after a reseed-style repair; the sync
checkpoint is the source of truth for whether source has moved since the last
acknowledged absorption.

## Agent state is not PR state

Agent runtime state is not a repository disposition reason. A Codex session
being busy, capped, assigned to another PR, or unable to continue in the
current branch must not decide whether a PR is closed, merged, parked, marked
superseded, or otherwise materially mutated.

Those conditions may be recorded as a handoff comment only:

```text
This agent session cannot continue work on this PR right now.
```

That statement does not imply repository disposition.

Forbidden close or disposition reasons include:

- this Codex session is working on another PR,
- Codex has an agent cap,
- the current session cannot continue,
- this branch is not the current active task,
- another PR is being worked first.

Valid PR dispositions must be based on repository facts:

- merged,
- mergeable and ready,
- parked for a later lane,
- superseded by a merged PR,
- duplicate of the chosen canonical PR,
- stale after useful content was extracted,
- invalid, unsafe, destructive, or wrong repository,
- owner-directed close.

Parking is not closure. If a PR is useful but belongs to a later lane, leave it
open unless the owner explicitly requests closure or the useful work has been
preserved elsewhere.

Every close must name the repository-level reason. When applicable, it must
also link the replacement PR, issue, branch, commit, or handoff that preserves
the useful work, and state whether the work can be reopened later.

Suggested disposition labels:

- `parked`
- `candidate`
- `ready-to-port`
- `needs-rebase`
- `later-lane`
- `superseded`
- `do-not-close-agent-state`
- `blocked-agent-cap`

## Promotion eligibility

A swarm slice is eligible for source promotion only after it has:

- green routed CI,
- fixture, golden, calibration, dogfood, receipt, schema, or docs proof
  appropriate to the claim,
- support-tier or status text updated when the public claim changes,
- no unsupported safety, UB-free, Miri-clean, site-execution, or policy-gate
  claim,
- no witness execution by default,
- no automatic comments,
- no blocking policy by default.

Promotion should recreate or cherry-pick the exact behavior onto current source
`main`. Do not merge a stale swarm branch directly into source.

## Source PR naming

Use `port(swarm #NN): ...` for promoted implementation slices, for example:

```text
port(swarm #90): reject stale copy range guards
```

Use normal public-surface prefixes for source-only work:

```text
docs(readme): add public unsafe-review badges
release: prepare 0.1.1 polish
```

## Promotion batches

When promoting more than one related swarm slice, add or update a short handoff
that records:

- swarm PR or commit,
- source PR,
- behavior carried over,
- proof carried over,
- support-tier or spec impact,
- release impact.

Promotion batches should stay reviewable. Prefer several narrow source PRs over
one large release-time cutover.

## Standing boundaries

The source repo remains conservative:

- `ReviewCard` remains the source of truth.
- Public badges and docs map to checked evidence.
- No support-tier promotion without proof.
- No default witness execution.
- No automatic comments.
- No source edits by the tool.
- No default blocking policy.
- No output may imply safety, UB-free status, Miri-clean status, site execution,
  or policy readiness without exact evidence.
