# Runtime-neutral repository orchestration protocol

This document defines the repository touchpoints that make issue-to-merge work
reviewable and recoverable. It does not prescribe an agent runtime, provider,
model, helper count, persona, messaging API, or fixed internal sequence.

Humans, Codex, Claude, other automation, and future runtimes may research,
implement, review, and reconcile work differently. They are compatible with
this protocol when they produce the same repository evidence and respect the
same authority boundaries.

For the ownership and disposition of each lifecycle surface, see the
[repository lifecycle surface map](LIFECYCLE_SURFACE_MAP.md). That map is the
inventory; this document explains the durable protocol without duplicating the
inventory.

---

## 1. Repository touchpoints, not runtime choreography

The repository standardizes outcomes at these touchpoints:

```text
useful live issue
-> accepted work contract when needed
-> one accountable writer per mutation surface
-> proportional local proof
-> reviewable PR with a bounded claim
-> exact-head review and hosted integration
-> merge under current policy
-> reconciliation and cleanup
```

The arrows describe evidence dependencies, not mandatory personas or a fixed
execution schedule. A narrow correction may pass through them in one short
session. A complex change may use research, planning, implementation, and
review helpers in parallel or in several iterations. One accountable person or
runtime may also carry the entire lane.

The repository decides acceptance through current source truth, deterministic
checks, and live GitHub policy. A model verdict, reviewer confidence, or local
green subset cannot replace those authorities.

## 2. Start from live authority

Every lane begins with a selected live GitHub issue or PR and the repository
artifacts it links. Before planning or writing:

- verify the current issue/PR disposition, exact base and head, and ownership;
- read the relevant spec, ADR, plan, policy, receipt, or closeout;
- confirm named files, commands, APIs, and checks still exist;
- inspect worktrees and active branches so a new writer does not race an
  existing mutation surface;
- run the source-divergence guard for routine swarm implementation.

GitHub owns the concurrent portfolio. `.allow` and linked repository artifacts
own durable contracts and graph visibility. Neither a local runtime queue nor a
cached planning summary selects the repository's next task.

If live state contradicts a plan, reconcile the contradiction before building.
Do not recreate an issue, spec, or plan merely because an example lifecycle
contains that step.

## 3. Compile a bounded work contract

A work contract must be detailed enough for a writer and reviewer to agree on:

- objective and user outcome;
- included surfaces and explicit non-goals;
- invariants and acceptance criteria;
- proof commands and integration expectations;
- risk, rollback, and cleanup;
- claim boundary and intentionally deferred follow-ups.

The GitHub issue may be sufficient for a small lane. When a stable
machine-readable contract is useful, follow
[SPEC-0044](../specs/UNSAFE-REVIEW-SPEC-0044-issue-linked-work-specs.md), the
[version-one schema](../schemas/issue-work-spec.schema.json), and the
[canonical example](../../plans/work-specs/examples/UNSAFE-REVIEW-WORK-1900.toml).
Validate that shape with `cargo run --locked -p xtask -- check-work-specs`.

The packet describes one delivery unit. It must not contain repository-wide
priority, a default goal, or live scheduling state. Validation proves structural
shape, not factual correctness, implementation completion, hosted status, or
merge readiness.

Planning is proportional. Mechanical documentation or test-only work can use a
short issue-backed contract. Public behavior, architecture, CI, policy,
release, or cross-repository work needs durable acceptance and rollback detail
before mutation.

## 4. Admit one accountable writer

One accountable writer owns each branch and overlapping mutation surface at a
time. Writer admission requires:

- a selected issue or accepted existing PR;
- an exact starting base;
- a dedicated clean worktree when the primary checkout is dirty or unrelated;
- an explicit read/write boundary;
- proof and stop conditions;
- known shared files or serialization constraints.

This rule protects authorship and rollback; it does not forbid collaboration.
Reviewers may investigate, propose patches, or identify repairs. Before a
reviewer or fixer mutates the branch, ownership must be explicit. The resulting
commit is authored mutation and invalidates review evidence tied to the prior
head.

When two lanes must edit one registry, snapshot, or generated projection,
serialize the writers or separate the source-of-truth structure first. Do not
resolve forced conflicts by racing rebases.

## 5. Choose execution shape proportionally

The runtime chooses its internal mechanics. Valid shapes include:

- one person or agent researching, writing, proving, and preparing the PR;
- bounded read-only helpers returning evidence to one writer;
- a dedicated writer plus an independent reviewer;
- several independent investigations followed by one integration decision;
- a long-running owner that changes lenses as the lane evolves.

Helpers are optional. Use them when independent evidence or parallel discovery
reduces risk; stay single-threaded when coordination would cost more than the
work. No repository correctness rule depends on a provider name, model tier,
agent count, stage persona, background mode, or particular messaging topology.

Whatever the shape, keep bulk logs and inventories outside the durable
contract. Return compact evidence: facts, paths, commands, status, uncertainty,
and the next bounded action.

## 6. Build and prove the selected seam

The writer implements one review-forward responsibility and uses proof that
matches its claim:

- behavior changes need observable success and meaningful failure coverage;
- refactors need characterization or existing drift locks on both sides of the
  extraction;
- policy and checker changes need accepted and rejected fixtures;
- docs changes need the relevant documentation, link, artifact, and static
  checks;
- generated projections must be regenerated from their source rather than
  hand-edited.

Run focused proof first, then the repository gate required by the accepted
contract. In this repository `cargo run --locked -p xtask -- check-pr` is the
deterministic core gate, while formatting, Clippy, workspace tests, rustdoc, and
hosted checks retain their separate authority. `check-pr` is not a release
readiness or safety verdict.

Report each requested check as pass, fail, or not run. A skipped policy result
is not a pass. Preserve `NOT_PROVEN` when an expensive or environment-specific
gate did not run.

## 7. Challenge the exact head

Review is a challenge against an immutable PR head, not a general approval of
the idea. A useful review asks checkable questions:

- Does the diff satisfy each acceptance criterion?
- Does a test fail if the intended property regresses?
- Does output wording cross the claim boundary?
- Did the writer touch an excluded surface or create a second truth?
- Are generated artifacts current and derived from the authoritative source?
- Are proof commands attached to the exact commit being considered?

Any relevant mutation after review makes that review stale. When feedback leads
to a repair, record the new head, rerun proportional proof, and obtain fresh
challenge for the changed head. A reviewer-to-fixer transition does not preserve
the old head's approval merely because the same participant understands the
change.

## 8. Publish, integrate, and merge

A PR is review-forward when its title, body, diff, and evidence describe one
responsibility and the reviewer can see both what is proven and what is not.
Use the repository PR template and link the governing issue and artifacts.

Before merge, verify live GitHub state:

- the head and base are unchanged from the reviewed values;
- required hosted checks are green under current policy;
- advisory checks are classified honestly;
- actionable review threads are resolved on the current head;
- the branch is mergeable and no active writer is still mutating it;
- local proof and hosted proof are not conflated.

Merge only through the repository's current policy and authorized style. A
green check subset, stale review, bot verdict, or local command does not grant
merge authority. Release publication, tags, deployments, credentials, and
direct source-repository promotion remain separately authorized actions.

## 9. Reconcile and clean

After merge:

- verify the merge commit and current `origin/main` ancestry;
- run focused post-merge checks where integration could change the result;
- update or close the governing issue only when its accepted contract is
  actually complete;
- record remaining work as a bounded issue, plan item, or closeout rather than
  widening the merged lane;
- remove only the clean worktree, branch, and scratch artifacts owned by the
  lane;
- preserve the primary checkout and ambiguous or user-owned state.

Use `cargo run --locked -p xtask -- cleanup-audit` when its advisory inventory
helps. It reports potential residue; it does not prove ownership or authorize
deletion. Build-cache guidance lives in [BUILD_CACHE_SETUP.md](BUILD_CACHE_SETUP.md).

## 10. Runtime adapters and manual compatibility

Runtime adapters translate this protocol into available operations. They may
provide specialized roles, prompts, tools, or shortcuts, but they do not become
repository source truth and must not persist live portfolio state.

A manual contributor must be able to complete the same lane with direct Git,
GitHub, Cargo, and shell commands. A broken or absent adapter falls back to:

1. inspect the issue and linked contract;
2. create an isolated worktree;
3. make the bounded change;
4. run the named proof;
5. open and review the exact-head PR;
6. merge when allowed;
7. reconcile and clean.

These are fallback touchpoints, not a mandate that every runtime use one fixed
internal sequence.

## 11. Verification catches wrong facts and wrong outcomes

Verification applies to facts, assumptions, and semantic acceptance.

- Objectify decisive claims with a command, fixture, source citation, or
  deterministic check when possible.
- Ask enumerated questions rather than requesting a general quality verdict.
- Verify the root premise before spending effort on downstream reasoning.
- Compare live state with planning artifacts; current issue, PR, code, and
  policy state win over stale summaries.
- Treat a green gate as necessary but not sufficient when it exercises the
  wrong property.
- Reconstruct the reason for broad golden or snapshot changes rather than
  accepting a mass refresh because the gate became green.
- Challenge value judgments and scope choices separately from factual claims.

Independent review is useful where the cost of a wrong judgment is meaningful,
but independence does not require a particular agent count. A distinct lens,
fresh evidence, and an exact-head boundary are the durable requirements.

## 12. Guardrails, not handcuffs

A work contract should constrain outcomes without prescribing an unnecessary
implementation recipe.

- A **guardrail** names a property that must remain true: do not drop an
  actionable finding, do not escape a configured root, do not create a second
  ReviewCard truth, do not expand scope silently.
- **Guidance** points to likely seams and nearby patterns while leaving the
  writer room to choose the cleanest implementation.
- A **handcuff** dictates internal choreography or a specific edit without a
  correctness reason.

Prefer guardrails expressed through tests, schemas, policy, or deterministic
checks. The accepted contract should carry the objective, non-goals,
assumptions to verify, acceptance proof, risk, rollback, and claim boundary.

## 13. Pair black-box and white-box evidence

Black-box proof asks whether a user-visible input produces the contracted
output. White-box proof asks whether the implementation preserved the internal
invariants that make that result trustworthy. Use both when either view alone
could hide a regression.

Examples:

- a CLI e2e test plus a focused parser or renderer test;
- an artifact-schema check plus inspection that every field still projects
  from `ReviewCard`;
- a hostile path fixture plus confirmation that no filesystem adapter resolves
  the path outside the configured root;
- a generated-document check plus confirmation that only its authoritative
  source was edited.

When helper evidence disagrees, collapse the disputed fact to the accountable
controller or owner. Escalate the specific contradiction, not the whole task.

## 14. Separate clearly correct repairs from stance decisions

Some feedback has a deterministic answer: a broken link, malformed schema,
missing regression assertion, stale generated projection, or violated accepted
invariant. An admitted writer may repair it within the lane and rerun proof.

Other feedback changes product or governance stance: detector behavior,
support posture, public claims, dependency trust, merge policy, self-unsafe
acceptance, release identity, or publication. Preserve those as explicit owner
or issue decisions. Do not disguise a stance change as cleanup.

When uncertain, state the competing contracts and the evidence each would
require. Keep the current behavior and claim boundary until the decision is
authorized.

## 15. Issue routing and external boundaries

Repository-owned issues and PRs are normal operating surfaces. File or update
them when authorized by the active lane and when durable coordination is
needed. Do not turn every observation into a repository mutation.

Findings about third-party repositories remain read-only evidence until a human
authorizes outward-facing publication. External pilots must not create comments,
issues, source edits, or witness executions merely to complete an internal
evidence matrix.

Security-sensitive output must remain bounded and redacted. Do not retain raw
hostile logs, arbitrary environment values, credentials, or secret-like
material as diagnostic evidence.

## 16. This repository's durable mapping

| Protocol surface | Repository authority |
|---|---|
| Operating contract and routing | `AGENTS.md` |
| Lifecycle ownership and overlap | `docs/contributing/LIFECYCLE_SURFACE_MAP.md` |
| Operational portfolio | Live GitHub issues, PRs, reviews, and checks |
| Durable graph | `.allow` plus linked specs, ADRs, plans, policies, and receipts |
| Issue-linked packet | SPEC-0044, its schema, and checked examples |
| Deterministic core gate | `cargo run --locked -p xtask -- check-pr` |
| Source/swarm posture | `cargo run --locked -p xtask -- source-divergence` |
| Worktree isolation | One verified branch/worktree per active mutation lane |
| Product truth | `ReviewCard` and its governed projections |

`unsafe-review-swarm` develops; `unsafe-review` publishes. Routine product,
evidence, fixture, dogfood, and repository work belongs in the swarm. Source
promotion, release preparation, and public package claims remain distinct.

The product claim remains advisory: unsafe-review finds unsafe Rust changes
missing a safety contract, guard, test, or witness. No workflow, model, review,
or green gate establishes proof, UB-freedom, Miri cleanliness, site execution,
calibrated precision/recall, release readiness, or permission for automatic
comments or source edits.

## Claim boundary

This protocol defines repository authority, handoff evidence, and lifecycle
touchpoints. It does not configure or launch agents, select models, require
delegation, choose the live portfolio, change merge protection, alter product
behavior, or prove that any orchestration approach is faster or more efficient.
