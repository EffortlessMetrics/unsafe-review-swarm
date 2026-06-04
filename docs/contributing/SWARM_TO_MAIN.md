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

## PR disposition policy

Agent runtime state and lane state are not repository disposition reasons. A
Codex session being busy, capped, assigned to another PR, unable to continue in
the current branch, or working in a different release lane must not decide
whether a PR is closed, merged, parked, marked superseded, or otherwise
materially mutated.

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
- another PR is being worked first,
- this PR is outside the current release lane.

Use this rule for lane mismatch:

```text
out-of-lane = defer / draft / blocked
not close
```

`blocked` is not `abandoned`. A blocked PR can preserve useful review context
while it waits for a concrete fix such as a missing secret, overbroad
permission, inaccurate allowlist, missing timeout, checkout posture issue,
failing CI, or required reviewer decision.

The Droid/MiniMax automation lesson follows the same rule. Aligned automation
work can be blocked or parked while the active lane stays elsewhere. Capture the
bot findings, validation gap, and next lane in a PR comment or handoff, but do
not reopen automation work solely to satisfy an unrelated lane unless the owner
asks.

Valid PR dispositions must be based on repository facts:

- merged,
- mergeable and ready,
- deferred, draft, blocked, or parked for a later lane,
- needs rebase or narrow rework,
- superseded by a merged PR,
- duplicate of the chosen canonical PR,
- rejected because it is invalid, unsafe, destructive, or wrong repository,
- abandoned after owner confirmation or documented non-response,
- unrecoverable after useful content was extracted or preserved elsewhere.

Parking is not closure. If a PR is useful but belongs to a later lane, leave it
open unless the owner explicitly requests closure or the useful work has been
preserved elsewhere.

Close only for one of these repository-level reasons:

- `duplicate`: another open PR is the canonical review packet for the same
  work.
- `superseded`: a merged PR, replacement PR, issue, branch, commit, or handoff
  preserves the useful work.
- `rejected`: the work is invalid, unsafe, destructive, in the wrong
  repository, or owner-rejected.
- `abandoned`: the owner confirms abandonment or documented follow-up attempts
  fail and no useful review context remains to preserve.
- `unrecoverable`: the branch or diff cannot be made reviewable without
  discarding the useful work, and that useful work has been preserved elsewhere
  if possible.

Every close must name one of those reasons. When applicable, it must also link
the replacement PR, issue, branch, commit, or handoff that preserves the useful
work, and state whether the work can be reopened later.

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

Normal promotion should recreate or cherry-pick the exact behavior onto current
source `main`. Do not merge a stale swarm branch directly into source for
routine promotion.

Exceptional source-history repair is different. If source is structurally behind
swarm and missing source-relevant reviewed history or tree state, use
`docs/contributing/SOURCE_HISTORY_CATCHUP.md`, preserve swarm ancestry, and
require merge-commit mode.

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

## History-preserving catch-up

Most source promotions should stay narrow and curated. When source has drifted
hundreds of reviewed swarm commits behind, or when source is missing
product-relevant swarm commits that already landed in the workbench, use the
history catch-up runbook instead of another curated copy or squash promotion.

A history catch-up PR must:

- merge `unsafe-review-swarm/main` into `unsafe-review/main` with a real merge,
- preserve swarm PR-squashed commits as ancestry,
- resolve conflicts explicitly,
- verify key source-relevant swarm commits are reachable,
- explain any remaining source/swarm tree differences,
- be merged with a merge commit.

Do not squash a history catch-up PR. If merge commits are blocked by repository
settings or branch policy, stop and fix that policy friction before merging.

An `ours` merge is only for pure ancestry bookkeeping when source tree state is
already known-equivalent. It is not valid when source is missing reviewed swarm
tree changes that need to land in source.

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
