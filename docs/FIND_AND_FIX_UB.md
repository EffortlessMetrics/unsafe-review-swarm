# Find And Fix UB-Risk Review Seams

This is the maintainer workflow for turning a changed unsafe seam into a
reviewable next action.

`unsafe-review` does not prove UB. It finds unsafe seams where UB is worth
investigating and tells you what evidence would make the seam reviewable.

Do not say:

```text
unsafe-review found UB.
```

Say:

```text
A safe caller can reach this unsafe operation without satisfying its invariant.
Here is the input or state.
Here is the minimal fix shape.
Here is the regression, witness route, or receipt that would add evidence.
```

## The Loop

```text
changed unsafe seam
-> ReviewCard
-> unsafe operation + invariant
-> evidence found / missing / weak
-> one next action
-> fix recipe
-> external witness route
-> receipt audit
-> outcome comparison
```

The goal is not to make `unsafe-review` decide the PR. The goal is to make the
next review step obvious and bounded.

## Public Workflow Contract

Use this workflow order when describing the maintainer path:

```text
first-pr -> pr-summary -> explain -> context -> witness-plan -> receipt audit -> outcome
```

That means:

- `first-pr` renders the advisory review kit.
- `pr-summary.md` is the maintainer cockpit for the top ReviewCard.
- `explain <card-id>` names the unsafe operation, invariant, evidence, and one
  next action.
- `context <card-id> --json` is a bounded handoff packet when the card is ready
  for agent work.
- `witness-plan.md` tells the reviewer which external route would add signal.
- `receipt audit` checks saved receipt metadata after a witness or human review
  happened outside `unsafe-review`.
- `outcome` compares before/after snapshots so the reviewer can see whether
  the review posture improved.

Call findings UB-risk review seams, unsafe-review gaps, or review gaps.
Do not say `unsafe-review` found UB.

## Minimal Command Path

From the branch you want to review:

```bash
unsafe-review doctor
unsafe-review first-pr --base origin/main
cp target/unsafe-review/cards.json target/unsafe-review/before.json
open target/unsafe-review/pr-summary.md
unsafe-review explain <card-id>
unsafe-review context <card-id> --json
open target/unsafe-review/witness-plan.md
unsafe-review receipt audit \
  --base origin/main \
  --format markdown \
  --out target/unsafe-review/receipt-audit.md
# After repair or receipt work:
unsafe-review first-pr --base origin/main
cp target/unsafe-review/cards.json target/unsafe-review/after.json
unsafe-review outcome \
  --before target/unsafe-review/before.json \
  --after target/unsafe-review/after.json \
  --format markdown
```

Use your editor or shell equivalent instead of `open` when needed.

## 1. Check The Environment

```bash
unsafe-review doctor
```

`doctor` checks whether the local checkout can produce useful review artifacts:
Git/base-ref visibility, Cargo metadata readiness, artifact directory
writability, and witness-tool hints. Missing witness tools are informational.
`doctor` does not run witnesses and does not make a policy decision.

## 2. Render The First Review Kit

```bash
unsafe-review first-pr --base origin/main
```

This writes:

```text
target/unsafe-review/review-kit.json
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/github-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/receipt-audit.md
target/unsafe-review/manual-candidates.json
target/unsafe-review/manual-repair-queue.json
target/unsafe-review/tokmd-packets.json
target/unsafe-review/lsp.json
target/unsafe-review/repair-queue.json
```

Save the starting card snapshot before edits when you want an outcome
comparison:

```bash
cp target/unsafe-review/cards.json target/unsafe-review/before.json
```

The bundle is advisory. It does not run witness tools, post comments, edit
source, run an agent, or enforce blocking policy.

## 3. Read The PR Summary

```bash
open target/unsafe-review/pr-summary.md
```

Start with the top card. A useful top card should tell you:

- the changed unsafe operation
- the operation family
- the safety obligation or invariant at risk
- what evidence was found
- what evidence is missing or weak
- one next action
- the `explain` and `context --json` commands
- the witness-plan and receipt-audit paths

If there are no changed cards, stop or widen the review scope intentionally. A
zero-card result is not a safety, UB-free, Miri-clean, or site-execution claim.

## 4. Explain One Card

```bash
unsafe-review explain <card-id>
```

Use `explain` to turn the card into a reviewer note. You are looking for this
shape:

```text
unsafe operation:
  the exact expression or declaration under review

invariant:
  the safety condition a safe caller needs to rely on

evidence found:
  contract, guard, reach, witness receipt, or route signals already visible

evidence missing:
  the smallest missing guard, contract, test reach, or witness receipt

next action:
  one concrete repair or evidence step
```

The card is actionable when you can name the safe caller or input/state that
reaches the unsafe operation and the invariant it fails to establish.

## 5. Choose The Fix Shape

Use the card family to choose the repair type. Keep the fix minimal and tied to
the missing evidence. For operation-family details, use
[ReviewCard fix recipes](explanation/fix-recipes.md).

Good repair shapes:

- add or move an executable guard that dominates the unsafe operation
- add or tighten a `# Safety` contract for the exact unsafe API or boundary
- add a focused regression test that reaches the safe caller path
- run an external witness and record receipt metadata
- replace the unsafe operation with a safe API when that preserves behavior

Bad repair shapes:

- comment-only text for a missing executable guard
- checking a different receiver, pointer, buffer, index, length, or owner
- checking after the unsafe operation
- broad refactors that hide the seam without improving evidence
- claiming Miri, `cargo-careful`, sanitizer, Loom, Shuttle, Kani, or Crux
  evidence without a receipt

For example, a `get_unchecked` card usually needs same-slice / same-index
bounds evidence that dominates the unsafe call and remains fresh after
reassignment or shadowing. A guard on another slice, a stale index guard, or a
post-check should not be treated as a repair.

## 6. Hand A Bounded Card To An Agent

```bash
unsafe-review context <card-id> --json
```

Use this only when the packet says the card is ready for bounded repair work.
For the full handoff path, see
[Bounded agent repair workflow](explanation/agent-repair-workflow.md).
The JSON includes:

- `agent_readiness`
- `missing_evidence`
- `allowed_repairs`
- `do_not_do`
- `verify_commands`
- `stop_conditions`
- `repair_queue`

An agent may work a card only when `agent_readiness.state` is
`ready_for_agent`. Other states are reviewer context, not edit tasks:
`requires_human_review`, `requires_witness_receipt`, and `unsupported`.

The agent should improve evidence for the exact card. It should not broaden the
scope, invent witness results, edit unrelated source, remove the unsafe seam
without review, or claim the code is safe.

## 7. Use The Witness Plan

```bash
open target/unsafe-review/witness-plan.md
```

The witness plan tells you which external route fits the card: Miri,
`cargo-careful`, sanitizers, Loom, Shuttle, Kani, Crux, or human deep review.
It also states route limits.

Run those tools outside `unsafe-review`. A suggested route is not evidence. A
saved receipt is evidence metadata only after the witness or review actually
happened.

After a witness run, record a receipt. A template is useful when the command was
run outside the tool:

```bash
unsafe-review receipt template <card-id> \
  --tool miri \
  --strength ran \
  --author reviewer/name \
  --recorded-at 2026-06-03T00:00:00Z \
  --expires-at 2026-09-03 \
  --summary "focused witness passed" \
  --command "cargo +nightly miri test <test-name>" \
  --limitation "focused regression only" \
  --out .unsafe-review/receipts/<card-id>-miri.json
```

Or import saved output when the adapter fits:

```bash
unsafe-review receipt import-miri <card-id> \
  --log target/miri.log \
  --author reviewer/name \
  --recorded-at 2026-06-03T00:00:00Z \
  --expires-at 2026-09-03 \
  --command "cargo +nightly miri test <test-name>" \
  --out .unsafe-review/receipts/<card-id>-miri.json
```

Receipt commands validate saved metadata or saved output shape. They still do
not prove the code is safe.

## 8. Audit Receipts

```bash
unsafe-review receipt audit \
  --base origin/main \
  --format markdown \
  --out target/unsafe-review/receipt-audit.md
```

Receipt audit checks whether saved receipt metadata still matches current card
identities, routed tools, strengths, expiry, and command hashes.
It does not run the witness command.

Open the audit before claiming evidence improved:

```bash
open target/unsafe-review/receipt-audit.md
```

If a receipt is stale, wrong-tool, expired, weaker than required, or mismatched
to the current card identity, treat it as audit metadata, not current witness
evidence.

## 9. Rerun And Compare Outcome

After the repair or receipt work:

```bash
unsafe-review first-pr --base origin/main
cp target/unsafe-review/cards.json target/unsafe-review/after.json
unsafe-review outcome \
  --before target/unsafe-review/before.json \
  --after target/unsafe-review/after.json \
  --format markdown \
  --out target/unsafe-review/outcome.md
open target/unsafe-review/outcome.md
```

Outcome comparison reads saved snapshots. It does not rerun analysis or make a
policy decision. Use it to answer:

- did the card resolve?
- did missing evidence shrink?
- did witness receipt strength improve?
- did a different card appear?
- did the top remaining card become clearer or noisier?

Treat the result as evidence movement, not a safety verdict.

Good outcome language:

```text
The bounds obligation moved from missing to present for card <card-id>.
The witness gap remains, so the next route is Miri with a focused test.
```

Bad outcome language:

```text
unsafe-review proved this unsafe block is sound.
```

## Review Checklist

Before asking for a PR change, confirm:

- the card is for changed code in the intended review scope
- the operation and invariant are concrete
- the missing evidence is about the same receiver, pointer, buffer, index,
  owner, or boundary as the unsafe operation
- the next action is one repair or one witness route, not a broad mandate
- any agent packet is `ready_for_agent`
- any witness claim has a current matching receipt
- outcome comparison shows evidence movement, not a safety verdict

The final maintainer sentence should be modest:

```text
I do not know whether this unsafe code is sound, but I know what invariant
matters, what evidence is missing, what fix shape is appropriate, what witness
would add signal, and whether the repair improved the review posture.
```
