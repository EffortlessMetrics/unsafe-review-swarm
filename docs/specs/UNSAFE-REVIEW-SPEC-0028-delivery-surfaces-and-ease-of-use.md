# UNSAFE-REVIEW-SPEC-0028: Coverage instrument and product boundary

Status: proposed
Owner: product
Created: 2026-06-06
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- plans/0.2.0/implementation-plan.md
Linked issues:
- none
Linked PRs:
- TBD
Support-tier impact: none (umbrella spec; child specs carry tier impact)
Policy impact:
- none

## Problem

`unsafe-review` has a strong evidence engine and a weak, mis-framed adoption
surface. The analyzer, ReviewCards, manual candidates, confirmation cues,
witness routing, and receipts are well specified and rail-checked. But turning
the tool on elsewhere still requires reading the spec stack, copying workflow
YAML, hand-editing policy ledgers, and stitching artifacts together — and the
docs imply `unsafe-review` is "the PR gate," which it is not and should not be.

This spec fixes the framing and routes the ease-of-use lane. It introduces no
analyzer behavior.

## What unsafe-review is

`unsafe-review` is the **unsafe coverage instrument**. Its job is to answer, with
checked artifacts, for a repo or a PR diff:

```text
- where is unsafe Rust touched?
- what unsafe obligation is involved?
- what evidence exists?
- what evidence is missing or weak?
- what changed versus baseline?
- what comment should a reviewer see, if any?
- what witness receipt would add evidence?
- what packet can an agent/LLM safely use?
```

The question it answers is **"is this unsafe seam reviewable?"**, not "is this
UB?". The product unit is **coverage**, not a verdict. A useful card says: this
unsafe operation changed; the obligation is bounds / initialization / stable
bytes / caller contract; contract evidence is present; guard evidence is weak;
test reach is missing; no matching witness receipt exists; this is new versus
baseline; this is worth one PR comment. Downstream tools consume that.

## What unsafe-review is not

`unsafe-review` does not own, and must not become:

```text
whole-CI orchestration            default blocking policy
the LLM reviewer                  automatic comment posting by default
generated-test execution policy   general UB verdicts
ripr / cargo-allow as a runner    Miri-clean or site-execution claims
```

It emits artifacts those systems consume. It does not become those systems.

## The product boundary

```text
unsafe-review = unsafe coverage instrument
ub-review     = LLM review layer / orchestrator on top of the sensor tools
```

`unsafe-review` is one of a **series of deterministic, fast, useful static PR
tools** (with `ripr`, `cargo-allow`, `tokmd`): each is cheap, runs on a diff,
and emits trusted coverage artifacts without executing the repo's code or making
a verdict.

`ub-review` is a **CI gate** built on that family plus LLM lanes. It lets a repo
keep its *mandatory* CI surface clean and simple, then dynamically adds the
**PR-relevant** gate items — composing the deterministic family (the whole set,
or a subset the user configures, or others) and running **LLM lanes** over their
coverage artifacts to do PR analysis, review, and gating. So `ub-review` owns
LLM review, comment posting, the blocking decision, generated tests, caches, and
runner routing; `unsafe-review` provides the high-integrity coverage substrate
and the **bounded optic** those LLM lanes read — coverage slots, do-not-do
rules, and forbidden-claim-checked comment-plan slots. `unsafe-review` is the
instrument, not the gate and not the LLM reviewer.

**Posting.** `unsafe-review` does not post by default. In its standalone PR-gate
mode, posting is an explicit opt-in and `unsafe-review` posts the planned
comments itself (the trusted-poster split-token / idempotency model). When run
inside `ub-review`, the LLM layer posts and `unsafe-review` only emits the plan.
`comment-plan.json` is the durable artifact in both deployments; who posts
depends on deployment, the plan does not.

`unsafe-review` ships a first-class **standalone PR-gate mode** for adopters who
want only this one tool in their CI: `first-pr`/`repo`, the optional
`--policy no-new-debt` exit hook (UNSAFE-REVIEW-SPEC-0030), and opt-in comment
posting make a complete, self-contained gate. That mode is real and supported.
It is a thin layer over the same coverage artifacts, not a separate product —
and the reference architecture, including the maintainer's own use, is
`unsafe-review` as a **component inside `ub-review`**, the LLM layer that
consumes its artifacts and owns review/posting/blocking. The two are not in
tension: both read the identical coverage substrate; only the consuming actor,
the posting actor, and the blocking decision differ.

The standalone gate mode also serves as the instrument's **dogfood and testing
surface**: running it on this repository's own PRs exercises the full coverage
artifact pipeline (cards, movement, comment-plan, manifest) against real diffs,
so the gate keeps the instrument honest end-to-end even where the production
consumer is `ub-review`. The gate mode earns its place as a self-test, not only
as an adopter convenience.

Docs that currently imply `unsafe-review` *is* "the PR gate" (as if that were
its whole identity) are corrected to: `unsafe-review` is the unsafe coverage
instrument; it offers a PR-gate mode and is also consumed as a lens inside a
larger gate such as `ub-review`.

`unsafe-review` is also one of a **family of sibling sensor tools** (with
`ripr`, `cargo-allow`, `tokmd`) that share interfaces and deliberately learn
from each other under the same orchestrator. The shared contracts (sensor CLI
shape, `<tool>-gate.json` manifest envelope, ledger/evidence taxonomy,
coverage-movement vocabulary, trust-boundary discipline, spec rails) and the
live bidirectional learning ledger are documented in
[`docs/interop/sibling-tools.md`](../interop/sibling-tools.md), mirrored in each
sibling repo. Child specs that define a shared artifact (SPEC-0030 baseline
movement, SPEC-0034 gate manifest, and the ledger schema) must co-design that
artifact with the sibling tools rather than invent a parallel format.

## Surfaces are consumers, not owned modes

There is one source of truth: ReviewCard and manual-candidate identity. Every
delivery surface is a projection consumed at a different lifecycle moment by a
different actor. They do not share a trigger or a cadence.

| Use case | Lifecycle moment | Consumer / actor | What unsafe-review provides | "Easy" means |
|---|---|---|---|---|
| 1. Repo badge | repo posture (push-to-main / scheduled) | README reader | numeric Shields-safe badge from baseline-aware coverage counts | one README line that stays fresh |
| 2. PR gate | PR-time | CI / orchestrator | review kit with new/worsened/resolved/inherited coverage | the orchestrator runs one command and reads one manifest |
| 3. PR line comments | PR-time | reviewer (posted by orchestrator) | bounded, deduped `comment-plan.json` anchored to coverage gaps | posting wrapper has a plan it can trust verbatim |
| 4. LSP feedback for LLMs | authoring-time (editor/agent loop) | a coding agent | stable context packet: coverage slots, allowed repairs, do-not-do, receipt route | obligations + cues + repairs for `file:line` in one query |
| 5. ub-review integration | meta-orchestration | the orchestrator + model lanes | a stable `unsafe-review-gate.json` manifest + structured artifacts | no markdown scraping; route by schema |

Shared invariant across all five: identity plus trust-boundary discipline. Every
rendered claim is deterministic or labelled a hypothesis; no surface claims
memory-safety proof, UB-free status, Miri-clean status, site execution,
calibrated precision/recall, or policy readiness. The invariant is not a shared
trigger or a shared gate shape.

## The coverage model

The core model is **coverage slots**, not just findings. Each card exposes these
machine-readable slots (child SPEC-0029 defines them):

```text
contract coverage        witness-receipt coverage   outcome movement
guard coverage           manual-candidate context   comment-plan status
test-reach coverage      baseline state             agent/LSP readiness
```

This is what makes badge, gate, comment-plan, LLM packet, and the ub-review
manifest all projections of the same measured coverage rather than separate
report formats.

## The ub-review contract

`ub-review` must not scrape markdown. `unsafe-review` emits one stable manifest
(child SPEC-0034) that points at the structured artifacts and summarizes
movement:

```json
{
  "schema_version": "unsafe-review-gate/v1",
  "summary": { "new_gaps": 2, "worsened_gaps": 1, "resolved_gaps": 3, "inherited_gaps": 91 },
  "cards": "cards.json",
  "comments": "comment-plan.json",
  "repair_queue": "repair-queue.json",
  "receipt_audit": "receipt-audit.json",
  "trust_boundary": "static unsafe-review evidence; not proof"
}
```

## Child specs mapped to the build sequence

```text
PR 1  SPEC-0028 (this)  product boundary: instrument vs orchestrator
PR 2  SPEC-0029         unsafe evidence coverage model (coverage slots)
PR 3  SPEC-0030         baseline and coverage movement (new/worsened/resolved/inherited)   [keystone]
PR 4  SPEC-0031         baseline-aware repo badge
PR 5  SPEC-0032         comment-plan hardening (coverage-gap anchored; no posting)
PR 6  SPEC-0033         LLM context packet (stable agent-facing optic)
PR 7  (SPEC-0027 amend) manual candidate authoring: candidate new / lint        [in flight]
PR 8  (shipped)         stable-byte getter-reentry coverage detector v1          [merged: swarm #1508]
PR 9  SPEC-0034         ub-review gate contract artifact (unsafe-review-gate.json)
PR 10 (release)         baseline-aware coverage usability release notes
```

Dependency order, not priority order. SPEC-0030 (baseline movement) is the
highest-leverage adoption unlock: without baseline-relative coverage movement,
every other surface inherits a tool that reports all pre-existing debt and gets
muted on mature repos. SPEC-0029 (coverage model) precedes it as the schema the
movement and all projections read. SPEC-0033 is independent and may proceed in
parallel.

## Non-goals

This umbrella does not introduce analyzer behavior, change ReviewCard or
manual-candidate identity, define an LLM review-authoring layer that free-writes
claims (any LLM-authored text in a child spec fills bounded, identity-anchored
slots with a forbidden-claims check, never free-write), or enable comment
posting, witness execution, source edits, or blocking policy by default.

## Trust boundary

Ease of use must not erode honesty. Each surface ships the boundary it projects
today: static unsafe contract review only; not memory-safety proof, not UB-free
status, not Miri-clean status, and not a site-execution claim unless a matching
witness receipt says so. The adoption work changes who can reach the coverage
evidence and how cheaply, not what the evidence claims.

## Machine check

Registered in `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` and routed from
`docs/specs/UNSAFE-REVIEW-SPEC-START-HERE.md`; validated by
`cargo run --locked -p xtask -- check-spec-status` and
`cargo run --locked -p xtask -- check-docs`.
