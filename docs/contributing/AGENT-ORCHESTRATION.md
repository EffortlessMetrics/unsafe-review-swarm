# Multi-agent orchestration doctrine

This document captures the multi-agent build doctrine as a portable playbook. The
pattern is repo-agnostic: it describes the spine, model routing, and hygiene rules
that make autonomous code-improvement loops reliable, cheap, and trustworthy. A
short section at the end maps each generic piece onto the specifics of this
repository.

---

## 1. Core idea

One orchestrator drives many cheap and mid-tier agents through a fixed per-work-item
lifecycle. Every stage is anchored to objective checks — real tests, a deterministic
gate, and dogfooded sibling tools — not agent self-assessment. The model routes,
compresses, and challenges checked artifacts; the deterministic gate decides
pass/fail. An agent's "this looks good" verdict is never the merge signal.

The economics mirror the product itself: cheap bounded sensors emit evidence, the
orchestrator compiles it, and the deterministic floor disposes.

---

## 2. The spine (per work item)

Each work item travels through the same ordered stages:

**Investigate.** A cheap agent reads the relevant code, flags, and docs. Output is a
bounded evidence packet: what bit, where, why it matters, uncertainty level, and a
concrete one-reason proposal. No essays.

**File issue and plan.** The orchestrator opens a tracked issue with: problem and
evidence, a bounded single-reason plan, acceptance criteria, drift-lock pointer (spec
or gate), and a trust-boundary statement (no proof/UB-free/Miri-clean claims).

**Verify (independent fact-check).** A second cheap agent checks the filed plan
against the code independently. It asks checkable questions: does this path exist,
does this symbol have this signature, is this proof command valid? Fix or flag before
the expensive build starts.

**Post issue update comment.** The orchestrator records the verification result on the
issue so there is a durable trail before the build begins.

**Write spec and repo-map.** A cheap agent writes or updates the spec — exact
files, symbols, line ranges, change shape, tests to add, proof commands. Where a
`docs/specs/` spec doubles as the documentation and drift-lock artifact, it is gated
by the spec-status and doc gates. The repo-map is the builder's starting context.

**Verify spec (triple-check).** Every file path, symbol, line number, and proof
command in the spec is checked against the current code. Any wrong reference is
fixed here rather than discovered mid-build. Catching an error at the spec stage
costs roughly 100x less than catching it during the Sonnet build.

**Build.** A mid-tier agent receives the verified spec and repo-map and implements
the change. It starts with an accurate map instead of burning tokens on discovery.
The builder must pass the targeted tests and the full deterministic gate.

**Adversarial review.** A reviewer agent checks specific named things from the spec:
does this diff violate the trust boundary, does the test cover the stated acceptance
criterion, does the gate pass? Not "is this good?" — checkable assertions only.

**Improve (loop).** On disagreement, the orchestrator sends a specific next step to
the same living builder agent via SendMessage rather than spawning a fresh agent.
Each callback names what to verify, which test to add, or which wording to tighten.
Context re-fires cheaply from cache; the loop can run multiple rounds.

**CI.** Watch the hosted CI run to completion before declaring local-green a merge
signal. Never trust local-green alone.

**Merge.** Squash-merge on green. Record the merge SHA.

**Clean.** Remove the temporary worktree and stale branch after the merge is
confirmed. Relocate heavy caches off constrained drives.

---

## 3. Two orchestration modes

### (a) Workflow — deterministic fan-out

Use for: discovery spine, parallel investigation of N targets, ephemeral tasks that
return a batch result.

Shape: kick off N agents in parallel, each investigating one target. Each returns
an evidence packet. The orchestrator collects the batch, deduplicates, and routes
each finding into the per-work-item spine above.

Important constraint: workflow-internal agents are not addressable after they finish.
Do not design a workflow stage that expects a later SendMessage into an agent that
ran inside a workflow. Use standalone background agents for anything you need to
message into later.

### (b) Long-running agent with SendMessage — stateful iteration

Use for: build → review → improve loops where context accumulates and re-firing
from cache is cheaper than re-discovery.

Shape: spawn the builder (or reviewer) as a standalone background agent with
`run_in_background: true`. When it completes, you receive a completion notification.
That notification is the event bus: use SendMessage to advance the living agent with
the next specific step. Keep the builder alive across rounds rather than spawning
fresh agents with no context.

The completion notification drives the next action. Do not sleep, poll, or
proactively check status — wait for the notification.

---

## 4. Model tiers

```
cheap (Haiku-class):  investigate, verify, spec, claim-boundary, cleanup, CI-triage
mid (Sonnet-class):   build, review
top (Opus-class):     hard arbitration, high-stakes judgment (detection calls,
                      trust-boundary calls, cross-repo conflicts)
```

The operating principle: "cheap models check, mid models build, top models judge."

Cheap passes are most valuable when they are plural with distinct roles:

- preflight: is this already landed, stale, or blocked?
- classification into a closed vocabulary
- plan refutation: attack the plan before implementing it (highest ROI)
- claim-boundary scans: does any output wording cross the trust boundary?
- cleanup audits: worktrees, branches, caches, stale targets

Escalate the specific conflict to the next tier — never the whole task.

On disagreement between two cheap passes, identify the exact point of conflict and
send that to the mid tier.

---

## 5. Why it is not self-grading

The review and verify loop is anchored to objective ground truth:

- **Deterministic gate.** A single pass/fail command sequence is the only merge
  signal. No model verdict substitutes for it.
- **Real tests.** The builder must make specific named tests pass. The reviewer
  checks that those tests exist and pass — not "looks good."
- **Dogfooding.** Run your own quality tools on your own diffs. If the project has
  a static-analysis or coverage tool, run it on every change it touches.

The reviewer asks checkable questions: "does this diff introduce a claim that
crosses the trust boundary?", "does test X cover edge case Y?", "does the gate
command listed in the spec exist and pass?" Those are verifiable facts, not
opinions.

This discipline only works if verification tools stay fast. A gate that takes
20 minutes does not live in the loop. Keep tests, static analysis, and the gate
quick enough for diff-level and PR-level use.

---

## 6. The spec stage and in-repo specs

Write or verify the spec before starting the expensive build. A cheap agent
triple-checks every path, symbol, line range, and proof command against the current
code. If any reference is wrong, fix it now.

Benefits:

- The builder starts with an accurate map instead of rediscovering the codebase.
  This saves the most tokens per build round.
- The spec doubles as documentation and as a drift-lock artifact: future changes
  that break the contract are caught by the gate.
- Builder, reviewer, tests, and gate all reference the same contract. There is one
  source of truth.
- Each part of the system is isolated, verifiable, testable, and documented
  independently.

In-repo specs live in `docs/specs/` (or the equivalent for your project). Gate them
with a spec-status check so they cannot drift silently.

---

## 7. Context caching and SendMessage efficiency

A 200k-token thread re-fires as roughly 20k input tokens via a cache hit. This
means: advance the living agent rather than spawning a fresh one.

Rules:

- Keep the builder alive across build and review rounds.
- Each SendMessage callback must be specific. Vague callbacks ("improve it") waste
  the cache benefit. Name the file, the line, the assertion, the test.
- Stable doctrine — specs, schemas, operating contracts — belongs in cacheable
  prefixes. Per-task content (diff, run ids, log excerpts) belongs in the suffix
  only. Do not bake timestamps or run ids into reusable prompts.
- Bulk content (full logs, large raw JSON, inventories) stays in subagent contexts.
  The main orchestrator context holds: objective, plan, decisions, artifact paths,
  validation status, and next action.

---

## 8. Issue routing and the hard boundary

Issue filing is autonomous only within the repos the organization owns. The rule:

**Own-org repos (EffortlessMetrics / EffortlessSteven):** file immediately with
evidence — problem, why it matters, concrete proposal.

**Dogfooded sibling tools the org owns:** a bug found while dogfooding a sibling
tool routes to that tool's dev repo. The dev repo is its `-swarm` repo if one
exists, otherwise its regular repo. Examples: `ripr` → `EffortlessMetrics/ripr-swarm`;
`tokmd` → `EffortlessMetrics/tokmd-swarm`; `cargo-allow` → `EffortlessMetrics/cargo-allow`;
`ub-review` → `EffortlessMetrics/ub-review`. Verify it is actually that tool's bug
(not a misuse on your side) before filing.

**Cross-pollination between siblings:** when a capability learned in one tool should
be adopted by another, file in the receiving tool's dev repo and add a row to the
shared learning ledger (see `docs/interop/sibling-tools.md`).

**Third-party or other-maintainer repos (not owned by the org):** log the finding
with evidence for human review. Never auto-file. The decision to open an issue on
an external repo belongs to the human.

This boundary is hard because external-repo filing is outward-facing publication
requiring human judgment. Collect friction points with evidence; batch third-party
findings into a "your call" list.

---

## 9. Standing hygiene

- Clean worktrees and stale agent branches after each merge. Do not let temporary
  worktrees accumulate.
- Relocate heavy caches (e.g. `CARGO_HOME`, npm caches) off constrained drives to
  avoid disk-full incidents mid-build.
- CI-watch every PR. Run the hosted check to completion before declaring green.
  Never trust local-green as a merge signal.
- LF line endings. On Windows, agents editing via Python or other tools can flip
  LF to CRLF whole-file. Verify EOL before merge with `git diff --check`.
- The trust boundary holds on every output surface. Every advisory tool in this
  category — static analysis, coverage, heuristic review — must maintain the same
  boundary: no proof, no UB-free claim, no Miri-clean claim, no site-execution
  claim, no calibrated precision/recall claim, no default blocking policy, no
  automatic comment posting, no source edits.

---

## 10. Adopt-in-your-lane

### Mapping template

Fill in for your project:

| Generic piece | Your value |
|---|---|
| Deterministic gate | `<command that decides pass/fail>` |
| Spec home | `<directory for behavior contracts>` |
| Drift-lock mechanism | `<gate that catches spec drift>` |
| Dogfood tools | `<tools you run on your own diffs>` |
| Issue tracker | `<org/repo>` |
| Worktree isolation | `<worktree path convention>` |
| Cheap model | `<Haiku-class equivalent>` |
| Mid model | `<Sonnet-class equivalent>` |
| Top model | `<Opus-class equivalent>` |

### Lane archetypes

- **Feature lane:** new behavior → spec → fixture/test → calibration (if
  analyzer) → gate pass.
- **Docs lane:** new or updated doc → check-docs pass → no behavior change.
- **Performance lane:** measure before optimizing → gate pass → no regression.
- **Refactor lane:** behavior-identical → gate pass → no second truth surface.
- **Dependency lane:** patch/minor safe; major needs targeted test for changed
  surface before merge.
- **Interop lane:** spec-first → adapter-free artifact contract → gate pass →
  learning-ledger row.

### Minimum checklist to stand up

1. One deterministic gate command that decides the merge.
2. A spec directory gated so specs cannot drift silently.
3. A claim-boundary check on every PR that touches user-facing wording or output.
4. A worktree convention (one worktree per task, clean after merge).
5. A model routing table (which phase uses which tier).
6. An issue-routing rule (own-org auto-file; third-party log-for-human).
7. A hygiene reminder: CI-watch, LF verify, cache relocation, cleanup audit.

---

## This-repo mapping

| Generic piece | This repository |
|---|---|
| Deterministic gate | `cargo run --locked -p xtask -- check-pr` |
| Spec home | `docs/specs/` (spec IDs `UNSAFE-REVIEW-SPEC-NNNN-*`) |
| Drift-lock mechanism | `check-spec-status`, `check-docs`, `check-calibration` |
| Dogfood tools | `unsafe-review` on own diffs; `ripr` on own diffs |
| Issue tracker | `EffortlessMetrics/unsafe-review-swarm` |
| Worktree isolation | `git worktree add -b <branch> <path> origin/main` |
| Cheap model | Haiku-class (roles in `.claude/agents/`) |
| Mid model | Sonnet-class (implementer role) |
| Top model | Opus-class (arbitration, detection calls, trust-boundary calls) |

Agent roles are defined in `.claude/agents/` (repo-preflight, claim-boundary,
plan-refuter, artifact-verifier, ci-log-triage, cleanup-auditor pinned to cheap;
implementer pinned to mid). Use them instead of inlining those jobs into the
orchestrator.

The operating contract is in `AGENTS.md`. The source-of-truth stack is:
`.rails/goals/active.toml` → linked plan item → linked spec in
`docs/specs/`. One PR-sized change, then run the proof commands from the plan item.

The trust boundary for this tool: advisory, no proof, no UB-free claim, no
Miri-clean claim, no site-execution claim, no calibrated precision/recall, no
default blocking, no automatic comment posting, no source edits. Every output
surface projects from `ReviewCard` as the single truth object.
