# Bounded Agent Repair Workflow

This workflow explains how to use `repair-queue.json` and
`unsafe-review context <card-id> --json` as a bounded handoff for humans and
agents.

`unsafe-review` does not run agents, edit source, run witnesses, post comments,
resolve cards, or prove UB. It gives the reviewer a card-scoped packet that can
make one repair attempt reviewable.

The reviewer remains responsible for choosing the card, limiting the file
scope, validating the patch, running any external witness, recording receipt
metadata, rerunning `unsafe-review`, and deciding whether the result improved
the review posture.

## Inputs

Start from a first-pr review kit:

```bash
unsafe-review first-pr --base origin/main
```

The agent-repair inputs are:

```text
target/unsafe-review/repair-queue.json
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/witness-plan.md
target/unsafe-review/receipt-audit.md
```

For one card, render the bounded packet:

```bash
unsafe-review context <card-id> --json
```

The queue and packet are copy-only artifacts. They do not grant automation
authority by themselves.

## Readiness Decision

Every repair-queue item and context packet carries `agent_readiness`.

| State | Can an agent edit? | Allowed use |
|---|---|---|
| `ready_for_agent` | yes | Try one bounded repair from `allowed_repairs`, inside reviewer-supplied file scope. |
| `requires_human_review` | no | Summarize the risk and ask a human to narrow or decide. |
| `requires_witness_receipt` | no source edit | Run or attach external witness evidence outside `unsafe-review`. |
| `unsupported` | no | Do not delegate from this packet. |

The consistency rule is strict:

```text
ready = true  -> state == ready_for_agent
ready = false -> state != ready_for_agent
```

If the state is not `ready_for_agent`, the packet can still be useful context,
but it is not an agent edit task.

## Queue Triage

Open `repair-queue.json` and answer four questions before delegating:

```text
Can an agent work this card?
What kind of work is allowed?
What must the agent not do?
What validation proves the repair improved evidence?
```

Use the queue buckets as sorting labels:

- `repairable_by_guard`: executable guard or discharge evidence may be the
  bounded repair.
- `repairable_by_safety_docs`: exact `# Safety` contract work may be the
  bounded repair.
- `repairable_by_test`: focused reach or regression test work may be useful,
  while preserving that a test is not site-execution proof.
- `requires_witness_receipt`: external witness evidence is missing.
- `requires_human_review`: human review must happen before edits.
- `do_not_auto_repair`: do not delegate as source repair work.

Cards may appear in more than one bucket when the reasons are distinct. For
example, a card can be repairable by guard evidence and still benefit from a
witness receipt. The bucket is not a verdict and not a policy decision.

## Freshness Check

Treat `repair-queue.json` and context packets as snapshots of one review kit.
Before delegating work, confirm the packet still describes the current card:

- rerun `unsafe-review first-pr --base origin/main` after meaningful source
  changes
- confirm the `card_id`, file, owner, operation family, and unsafe operation
  still match the card you intend to repair
- regenerate `unsafe-review context <card-id> --json` from the current review
  kit rather than reusing an old packet
- stop if the card disappeared, split into different cards, moved to a
  different unsafe operation, or changed missing evidence enough that the old
  `allowed_repairs` no longer apply

Freshness does not prove the card is correct. It only prevents an agent from
repairing stale instructions for a different unsafe seam.

## Packet Fields To Copy

When a card is `ready_for_agent`, copy these fields into the agent task:

- `card_id`
- `operation_family`
- `source_context.unsafe_site`
- `missing_evidence`
- `allowed_repairs`
- `do_not_do`
- `verify_commands`
- `stop_conditions`
- `repair_queue`
- the trust boundary

Do not ask the agent to infer scope from the whole repository. Give it explicit
files or fixture directories it may edit.

## Allowed Repairs

`allowed_repairs` names repair shapes for the current card only. Typical shapes
are:

- add or move an executable guard for the same receiver, pointer, index, buffer,
  length, owner, or callee named by the card
- add or tighten a `# Safety` contract for the exact unsafe API or boundary
- add a focused test that reaches the safe caller path or regression condition
- replace the unsafe operation with a safe API when that preserves behavior and
  stays inside the allowed scope

An allowed repair must improve the card's named missing evidence after rerun.
It is not enough for the patch to look safer in a general sense.

## Do-Not-Do Rules

Copy `do_not_do` with the task. The standing boundaries are:

- do not suppress this card as the repair
- do not replace executable guard or discharge evidence with a comment
- do not edit unrelated unsafe sites
- do not broaden the file scope
- do not invent witness results or say witnesses were run
- do not claim proof, UB-free status, Miri-clean status, site execution,
  calibrated precision/recall, policy readiness, or automatic safety repair
- do not post comments or block a PR from this packet

Deleting the unsafe operation is not automatically a repair. Replacing unsafe
code with a safe API can be a good repair only when the packet allows that
shape, the behavior is preserved, and the reviewer validates the result.

## Stop Conditions

An agent should stop and hand the work back when:

- `agent_readiness.state` is not `ready_for_agent`
- the needed repair is outside the reviewer-supplied file scope
- the packet points to human deep review, broad FFI ownership review,
  unsupported provenance, macro/cfg ambiguity, or receipt-only work
- the card identity, unsafe operation, or missing evidence no longer matches
  the current review kit
- the repair would require a broad refactor, public API change, ABI decision, or
  policy decision
- the verification command fails in a way unrelated to the patch
- rerun creates new or noisier ReviewCards that the packet did not cover

Stop conditions are a success path for bounded work. They prevent an agent from
turning one ReviewCard into an unreviewable rewrite.

## External Witness Receipts

`requires_witness_receipt` means the remaining work is outside the source-edit
packet. The reviewer should:

1. Open `target/unsafe-review/witness-plan.md`.
2. Run the suggested witness tool outside `unsafe-review`.
3. Record receipt metadata only after the witness or human review happened.
4. Audit the receipt against the current card identity.

Example receipt flow:

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

unsafe-review receipt audit \
  --base origin/main \
  --format markdown \
  --out target/unsafe-review/receipt-audit.md
```

An agent may help format a receipt only from actual command, output, author, and
time data supplied by the reviewer. It must not fabricate witness execution or
turn a suggested route into evidence.

## Handoff Template

Use a prompt like this for a `ready_for_agent` card:

```text
Use the attached unsafe-review context packet.
Address card <card-id> only.
Allowed file scope: <paths>.
Add or expose the missing evidence named by the card.
Use only the packet's allowed_repairs.
Copy and obey do_not_do and stop_conditions.
Run only the listed verify_commands that are available locally.
Return the patch summary, commands run, and whether the ReviewCard evidence
improved after rerun.
```

Include these boundaries:

```text
Do not suppress this card as the repair.
Do not edit unrelated unsafe sites.
Do not claim UB, safety, Miri-clean status, site execution, witness execution,
witness adequacy, calibrated precision/recall, policy readiness, or default
blocking.
Stop if the repair needs human review, external witness work, unsupported
provenance reasoning, broad FFI ownership review, or files outside scope.
```

## Validation

Before accepting an agent patch, rerun the review kit and compare snapshots:

```bash
cp target/unsafe-review/cards.json target/unsafe-review/before.json
# apply the reviewed patch
unsafe-review first-pr --base origin/main
cp target/unsafe-review/cards.json target/unsafe-review/after.json
unsafe-review outcome \
  --before target/unsafe-review/before.json \
  --after target/unsafe-review/after.json \
  --format markdown \
  --out target/unsafe-review/outcome.md
```

The patch improved review evidence only when the relevant card resolved,
weakened missing evidence shrank, witness receipt strength improved, or the next
action became narrower without creating unrelated new cards. A passing test can
support the review, but it is not a proof that the unsafe site executed unless a
matching witness or reach receipt says so.

Good outcome language:

```text
The same-buffer UTF-8 guard is now visible for card <card-id>.
The card still lacks a witness receipt, so the next route remains external.
```

Bad outcome language:

```text
The agent proved this unsafe code is sound.
```

## Relationship To Dogfood

Use `docs/dogfood/agent-repair-experiments.md` when recording whether a
card-scoped handoff was a good agent task, a bad agent task, human-only, or
uncertain. Dogfood records usefulness; they do not calibrate precision/recall
or prove safety.
