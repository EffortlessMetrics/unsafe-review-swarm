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

## 11. Why this works — designing for a ~12% error rate

The whole spine is error-rate engineering. Assume roughly one in ten of any
agent's claims is wrong — including the orchestrator's own memory. A workflow
that trusts single assertions accumulates that error; a workflow built to catch
it converges. The mechanisms, in priority order:

- **Objectify the claim.** Turn a claim into a check — a grep, a command, a
  deterministic gate — and it is ground truth (~0% error) regardless of which
  agent runs it. A check beats a verdict. Do this first, always. The decisive
  fact ("does this symbol have other consumers?") is a grep, not an opinion.
- **Different angle, not different agent.** The same warm agent catches its own
  errors when re-aimed: verify the diff *against the plan*, with a *new* test
  set, or from a different vantage (attacker, downstream consumer, skeptic).
  "Look again, is it right?" re-samples the same prior and launders the error;
  a different angle is a genuinely independent draw — at cache-warm cost, not a
  re-spawn. Reserve a fresh independent agent for irreducible judgment only.
- **Ask for enumerated actions, never "is it good?"** "Is it good/done/right?"
  triggers self-confirmation. Ask for tasks that produce an artifact: list the
  test surfaces and whether each goes red on revert; run the coverage tool;
  cite the controlling spec; point to the duplication. Name the specific
  assumptions to check against the code; do not leave grounding to discretion.
- **Verification includes reviewers and the controller.** A reviewer's verdict
  is also a ~12% claim. The controller checks the *decisive fact* before an
  irreversible step (merge), not the verdict.
- **A green PR can still prove the wrong property.** Passing every gate is
  necessary, not sufficient: a change can be green — fmt, clippy, tests, the
  full deterministic gate all pass — and still demonstrate the wrong thing. An
  implementer can honestly report "deviations: none" while having built a case
  that satisfies the *letter* of the brief and misses its *point* (e.g. a
  "resolved gap" fixture that resolves by deleting the unsafe rather than by
  adding the review evidence that was supposed to be rewarded — a near-tautology
  that passes CI but proves nothing the lane cares about). The controller's job
  is **semantic acceptance**: does this artifact actually demonstrate the
  intended product property? Check that against the brief's *intent*, not just
  the green check. Green CI ≠ correct outcome.
- **Your *weightings* can be biased, not just your facts.** Verification applies
  hardest to your own reasoning. Beyond "is this fact right?", ask "is my
  *value-weighting* skewed?" — a standing lean (for example, toward
  minimal/lean/external choices) is a systematic bias, not a one-off error, so
  surface it and invite refutation of the weighting, not only the facts. And
  verify the **root premise** first: a confident chain built on a wrong premise
  is wrong all the way down, so correcting the root collapses everything
  downstream of it.
- **Live state and owner decisions beat stale planning docs.** A planning doc
  (or your own earlier summary) is a ~12% claim about a *moving* repo, usually
  staler than it looks — by the time you act, PRs have merged and decisions have
  been recorded. When a doc conflicts with current state: verify the live
  PR / issue / commit / spec state first; identify which guidance is stale (it
  may propose work already landed, or reopen a settled call); preserve explicit
  owner decisions already recorded (a merged ADR, a parked PR, a spec status);
  continue only on the *live* gaps; and **ask before reversing a governance
  decision** — do not silently un-park, re-flip, or rebuild a settled call on
  the strength of a stale doc. Long unattended sessions hit this repeatedly: the
  live repo is the source of truth, the doc is a hypothesis.
- **Catch early; keep diffs small.** The cost of a wrong claim scales with how
  late it is caught (a spec edit, then a build cycle, then a shipped
  regression). A small diff carries few claims, so it is catchable; and a
  correction is a smaller surface than the original write, so the loop shrinks
  toward zero residual instead of oscillating. One reason per PR is an
  error-rate strategy, not just tidiness.
- **Write every correction down.** A drift-lock test plus a spec line plus a
  changelog entry turns each catch into a zero-cost inherited fact *and* a
  permanent regression catch. The accumulated docs/tests are the set of errors
  already caught; that set only grows, so per-PR convergence becomes a
  project-level ratchet — monotonically harder to regress.

The payoff: well-designed specs and tests are *rails*. Rails are what let cheap,
~12%-error agents run fast and unattended — the rails catch the one-in-ten
derailment cheaply and early (a red test, a failed gate, a spec-traced
contradiction). Without them you must hand-verify everything, which does not
scale. The real deliverable is therefore the *quality* of the test and spec
design, not "more agents."

## 12. Guardrails, not handcuffs

Give builders direction, guardrails, and guidance, then iterate. Do not hard-lock
them to specific practices or files.

- A **handcuff** prescribes the exact *how* ("implement it in this function").
  It is brittle (the builder often finds a better seam and ignores it) and
  beside the point (it does not prevent the failures that matter), and it
  discards the builder's judgment.
- A **guardrail** is a boundary on the *outcome* ("never drop an actionable
  finding", "do not cross the trust boundary", "do not expand scope silently",
  "every surface still projects from the one truth object").
- **Guidance** points at the goal and the principle to apply; the builder owns
  the implementation.

Prefer guardrails expressed as tests or gates — an objective boundary catches a
violation no matter how the builder got there. A brief should carry: goal,
non-goals, guardrails on outcomes, the specific assumptions to verify against
the code, and the proof commands — not a step-by-step recipe.

Two corollaries learned the hard way:

- **Specs and plans are directional hypotheses, not binding axioms.** A spec can
  be wrong. A builder that faithfully implements a wrong premise produces a
  wrong result that passes its own re-blessed checks. Verify the premise against
  the data before building on it; when verification falsifies it, update the
  spec rather than shipping to it.
- **Beware the re-blessed gate.** A green gate proves only that output matches
  expectations — which a builder can rewrite. When a change edits the
  expectations (goldens, calibration) at scale, verify the new expectations are
  *correct*, not merely that they pass.

---

## 13. Black-box / white-box pairing and collapse-to-root

Two complementary investigation modes compose into an efficient improvement loop
for any analyzer class (detector correctness, false-positive reduction,
evidence-precision):

**Black-box dogfood** — run the shipped tool against real, unseen crates and
review the output against your own expectations. This surfaces *instances* of a
problem: "card A on crate X looks like a false positive," "card B on crate Y is
missing." It finds the symptom.

**White-box detector audit** — given the symptom, read the detector code with the
failing case in hand and ask: what mechanism caused this? Is there a more general
class of case the detector fails on? This finds the systemic *root cause*.

The pairing matters: black-box alone finds isolated instances and prompts
instance-level fixes. White-box alone finds code paths but not real-world
triggers. Pairing finds the class, which lets you fix the mechanism rather than
each instance.

**Collapse-to-root** is the application of this pairing: when N findings share a
mechanism, fix the mechanism, not the instances. In practice this means:

1. Run dogfood, note the symptoms.
2. Audit the detector for the mechanism.
3. Name the root cause as a discipline or property (see SPEC-0005 appendix).
4. Write one fix at the mechanism level.
5. Add negative-control fixtures that would catch the class before any future
   regression.

Example from this project: ~30 false-positive instances across the fixture suite
collapsed to ~5 root mechanisms (scope gate, definition-vs-call, same-origin
discharge, string/comment masking, path-segment anchoring). Fixing the mechanisms
cleared all instances simultaneously and left fixture-encoded regression guards.

The controller-context is scarce when doing large fixture batches. The monolithic
`policy/calibration.toml` serializes fixture PRs — parallel fixture additions
collide on the file. The operational mitigation is to serialize fixture PRs in the
controller (one merge before the next begins), or to have the controller register
the new entry at merge rather than baking it into the PR diff.

---

## 14. The clearly-correct / stance partition (autonomy contract)

Not all findings in an improvement pass are equally certain. The autonomy contract
separates findings into two buckets:

**Clearly-correct fixes** — the detector is demonstrably wrong for this case: it
fires on safe-context code, on function definitions, on string/comment text, or on
a different binding than the one it should be checking. A cheap agent can and
should ship these without owner input, because the fix has no product-stance
implication.

**Stance decisions** — the fix requires a product judgment: "should a guard on X
discharge obligation Y even in this context?" or "should the tool be stricter
here, or is this an intentional advisory stance?" These cannot be autonomously
resolved. File them with evidence, propose the recommended stance, and escalate.

The operational rule: default to the more conservative (retain the card) stance
when unsure which bucket a finding belongs in. Evidence: a session in which the
owner ratified all five escalated stance recommendations and the clearly-correct
fixes were shipped in parallel without owner roundtrips.

This partition is reusable for any correctness improvement loop, not just unsafe
analysis. Autonomous throughput on clearly-correct items; human decision on stance
items; evidence-packets on escalations (not raw diffs).

---

## 15. The two agent failure modes: the organizing frame for the rails

Every safeguard in this doctrine maps to one of two predictable agent failure
modes. Naming them explicitly makes the whole apparatus legible:

**Agents overclaim.** An agent will confidently assert facts it cannot verify,
report a green gate as proof of correctness, bless goldens that mask regressions,
or produce output that drifts across the trust boundary. Overclaiming is not
laziness — it is the natural output of a generation process that is not internally
checked.

**Agents miss context.** An agent will act on incomplete information: a stale
spec, a diff that does not include the file that matters, a symbol it named but
did not read, a detector that fires without checking scope. Missing context is
also natural — agents act on what they are given, not on what they were not
given.

The whole spine of this doctrine is two defenses against these two failures:

**Anti-overclaim apparatus:**
- claim-boundary gates (no proof / UB-free / Miri-clean / site-execution claims
  on any output surface)
- distrust your own green / wholesale-bless verification (§15.a below)
- adversarial verify passes (different-angle review, plan-refuter, fact-check)
- the trust boundary hardwired into every spec and gate

**Anti-miss-context apparatus:**
- the detector-discipline contract (D1–D5) — forces each detector to explicitly
  check scope, receiver, call-vs-definition, span type; context that would be
  implicit in a type-aware analyzer must be explicit here
- repo-preflight and issue-factcheck agents — cheap independent reads before an
  expensive build starts
- the controller owning the seams: base freshness, calibration, gate sequence,
  worktree hygiene

When a new safeguard is proposed, identify which failure mode it defends against.
If it defends against neither, it is not load-bearing.

### 15.a Wholesale-bless verification (anti-overclaim)

A green `check-pr` plus a confident agent report is not proof. A mass
`UPDATE_GOLDENS=1` / `bless-goldens` run that touches many goldens simultaneously
is a yellow flag: the controller must reconstruct *why* the mass change is a
correct mass-correction — from the source diff, not from the green — before
treating it as valid.

Two failure modes:

- **Correct mass-correction** (e.g. stringify-vs-call stance change affecting 238
  fixtures) — green after bless is correct because the stance was deliberate and
  documented.
- **Laundered regression** — a change that breaks a property the fixtures were
  meant to catch; bless silences the failure instead of fixing it.

The controller's job is to distinguish the two. Ask: "what property did the old
goldens prove, and does the new golden still prove it?" If the answer requires
reading the diff rather than trusting the green, that is the right instinct. The
explanation is the proof; the green is the precondition.

### 15.b Agents are reliable; seams are fragile (anti-miss-context)

Across ~40 implementer PRs in the card-correctness session, the failures were not
wrong logic inside an agent but coordination seams between them:

- stale-base diffs (the agent built against a commit that had already moved)
- calibration merge conflicts on the shared `policy/calibration.toml`
- `cargo fmt` not run inside `check-pr` (the gate passes while a fmt-only push
  would fail in CI)
- a worktree leaking state onto the main checkout

The controller's value is owning the seams — verifying base freshness,
serializing calibration merges, running the full gate sequence, auditing worktree
hygiene — not implementation. Implementation is the cheap part; seam integrity is
the scarce part.

### 15.c Serialize against forced serialization

When shared registry files (one `policy/calibration.toml`, count snapshots,
badge files) make parallel fixture PRs always conflict at merge, the right
response is to serialize the merges deliberately — slower wall-clock, sane
context — and fix the structure (per-fixture registration files, issue #1712)
rather than trying to out-muscle the conflicts with rebase loops.

Operational rule: when N parallel PRs all need to touch the same file, serialize
the merges; do not parallelize the queue. The wall-clock cost of serialization is
a one-time tax; the context cost of repeated rebase conflicts is recurring.

### 15.d The verification ladder (anti-overclaim: stop trusting a single green check)

Trust is a ladder. Each rung catches what the rung below is blind to. No single
green check is proof; the ladder is how you stop believing any one of them.

| Rung | What it proves | What it is blind to |
|---|---|---|
| **fixture green** | the cases the author imagined | assumptions the author did not know they were making |
| **check-pr green** | gates + calibration consistency | `cargo fmt` + `cargo clippy`; whether the GOLDENS themselves are correct |
| **claim-boundary clean** | no forbidden wording on any surface | wrong logic with clean wording |
| **reconstruct-the-argument** (on a wholesale bless) | the mass change is a correct mass-correction, not a laundered regression | nothing — but it is manual and judgment-bound |
| **dogfood (real crate, known)** | behavior on real code in the corpus | what that specific crate does not exercise |
| **fresh crate (never-seen)** | the FP class is actually gone on unseen code | the next unhunted class |

Operational rule: after any mass golden bless, claim that it is correct only after
completing the reconstruct-the-argument rung. After any behavior change, claim it
is correct only after completing at least the dogfood rung. The fixture suite and
the gate alone are insufficient.

This is the operational answer to "agents overclaim": the ladder is how you stop
believing a single green check.

---

---

## 16. The orchestrator role: judgment, not generation

### The orchestrator is not a tech lead

The orchestrator is closer to a PM or Director of Technology than to a senior IC.
Its job is:

- **scope** — what is in this PR, what is not, where the boundary is
- **sequencing** — what runs first, what waits, what is blocked
- **risk** — what could go wrong, what the fallback is
- **decisions** — which of two plausible approaches is the right one
- **escalation** — when to stop and ask the owner rather than guessing
- **resource allocation** — which model tier, which agent, which parallel vs. serial
- **verification** — did this actually prove what we need it to prove
- **saying no / not now** — declining scope creep, deferring non-critical work

The orchestrator does NOT write the hard parts. It does not implement the
detector, author the fixture, or draft the spec body. Those are generation tasks,
and generation is effectively unlimited. Judgment is the scarce, load-bearing
resource.

### Generation is unlimited; judgment is scarce

This is the asymmetry the whole doctrine is designed around. A coding agent can
generate a correct implementation with high probability given an accurate spec and
a clear acceptance criterion. Generating the implementation is cheap. What is
scarce:

- Knowing which of two correct implementations is right for this product
- Deciding that a green gate proves the wrong property
- Recognizing that an agent's confident assertion is an overclaim
- Determining whether a stance question should be autonomously resolved or
  escalated to the owner
- Choosing what to do when a spec and the live repo disagree

The orchestrator's finite attention must be spent almost entirely on judgment
calls. All generation is offloaded to agents. When the orchestrator finds itself
drafting implementation details, that is a misallocation: it is doing generation
work that an agent could do, while depleting the attention budget available for
the judgment work that only the orchestrator can do.

Corollary: a brief should carry goal, non-goals, guardrails, assumptions to check,
and proof commands — not a step-by-step implementation recipe. The recipe is
generation; the guardrails are judgment.

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
