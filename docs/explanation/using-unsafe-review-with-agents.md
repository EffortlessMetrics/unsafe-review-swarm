# Using unsafe-review with a coding agent

`unsafe-review` does not run agents, edit source, run witnesses, post comments,
or resolve cards. It produces a bounded context packet for one card that an
agent can use to make one reviewable repair attempt.

This guide is for agent implementers: teams integrating `unsafe-review` output
into an agentic coding workflow. If you want the reviewer-facing workflow, see
[Bounded agent repair workflow](agent-repair-workflow.md). For concrete packet
JSON, see [Agent packet examples](agent-packet-examples.md).

## The bounded-card model

`unsafe-review` works at card scope, not repo scope. An agent should receive a
packet for exactly one card and attempt exactly one bounded repair. It should
not be asked to "fix the unsafe code in this repo" — that framing bypasses the
card-scoped discipline that keeps repairs reviewable.

The task model is:

```
1. Run: unsafe-review first-pr --base origin/main
2. Read: target/unsafe-review/repair-queue.json
3. Choose one card with agent_readiness.state == "ready_for_agent"
4. Fetch: unsafe-review context <card-id> --json
5. Hand the packet + explicit file scope to the agent
6. Agent makes one bounded repair
7. Reviewer: rerun first-pr, compare outcome, validate
```

An agent MUST NOT iterate across all cards automatically, make broad edits
outside the supplied file scope, or skip the reviewer validation step.

## Getting the context packet

For a known card ID:

```bash
unsafe-review context <card-id> --json
```

Or from a running first-pr review:

```bash
unsafe-review context --file path/to/file.rs --lines 42-67 --json
```

The `--json` flag writes the packet to stdout. The packet is a copy-only
artifact: it does not grant authority to edit, run witnesses, or post comments.

## Readiness routing

Every packet carries `agent_readiness`. Route on `state` before delegating:

| State | What it means | Allowed agent action |
|---|---|---|
| `ready_for_agent` | A bounded source edit may address the missing evidence | Try one repair from `allowed_repairs`, inside reviewer-supplied file scope |
| `requires_human_review` | The card needs human judgment before edits | Summarize the obligation and ask a human to narrow or decide |
| `requires_witness_receipt` | The remaining work is outside source edits | Route to external witness tooling; do not delegate as an edit task |
| `unsupported` | No repair path from this packet | Do not delegate |

The consistency invariant is strict:

```
ready = true   → state == "ready_for_agent"
ready = false  → state != "ready_for_agent"
```

If a packet says `ready = false`, it may still be useful context — but it is
not an agent edit task.

## What the packet carries

Every context packet (see `agent-packet-examples.md` for full JSON shapes)
carries:

- **`card_id`** — stable identifier for this card; include it in every agent
  prompt so the agent cannot drift to a different card.
- **`operation_family`** — the unsafe operation class (`raw_pointer_read`,
  `vec_set_len`, `maybe_uninit_assume_init`, etc.).
- **`source_context`** — a bounded context window with the unsafe site, nearby
  contract/guard summaries, and related test mentions. Keep scope to this
  window; do not tell the agent to search the whole repository.
- **`missing_evidence`** — the exact missing evidence the repair must address.
  This is the correctness target: the repair should cause a rerun to report
  improved evidence for this card.
- **`allowed_repairs`** — card-scoped repair shapes derived from the operation
  family and missing obligations. Typical shapes:
  - add or move an executable guard for the same receiver or pointer named by
    the card
  - add or tighten a `# Safety` contract for the exact API or boundary
  - add a focused test that reaches the safe caller path or regression condition
  - replace the unsafe operation with a safe API when that preserves behavior
    and the packet allows it
- **`do_not_do`** — standing boundaries; copy these into every agent prompt (see
  below).
- **`verify_commands`** — commands the agent should run after patching. These
  are suggested commands from the card; `unsafe-review` does not execute them.
- **`stop_conditions`** — conditions under which the agent should stop and
  return the work to the reviewer (see below).
- **`repair_queue`** — compact bucket labels (`repairable_by_guard`,
  `repairable_by_safety_docs`, `repairable_by_test`, `requires_witness_receipt`,
  `requires_human_review`, `do_not_auto_repair`) for sorting and routing.
- **`agent_readiness`** including **`requires_witness_receipt`** — a flag that
  signals whether the card needs external witness evidence before or alongside a
  source edit. If `true`, do not route as a pure edit task.
- **`baseline`** — the current card class and evidence state; the agent needs
  this to understand what "improvement" means after rerun.

## Do-not-do rules (copy these into every agent prompt)

```text
- Do not suppress this card as the repair.
- Do not replace executable guard or discharge evidence with a comment.
- Do not edit unsafe sites outside the reviewer-supplied file scope.
- Do not broaden the file scope.
- Do not invent witness results or claim witnesses were run.
- Do not claim proof, UB-free status, Miri-clean status, site execution,
  calibrated precision/recall, policy readiness, or automatic safety repair.
- Do not post comments or block a PR from this packet.
- Do not delete the unsafe operation unless the packet explicitly allows it and
  the behavior is preserved.
```

These boundaries exist because an agent asked to "fix unsafe code" will tend to
either suppress cards (deleting them without addressing the obligation) or make
broad changes that are hard to review. Both outcomes are worse than leaving the
card open.

## Stop conditions

An agent should stop and return the work to the reviewer when:

- `agent_readiness.state` is not `ready_for_agent`
- the needed repair is outside the reviewer-supplied file scope
- the packet points to human deep review, FFI ownership review, unsupported
  provenance, macro/cfg ambiguity, or receipt-only work
- the card identity, unsafe operation, or missing evidence no longer matches the
  current review kit (the packet is stale — regenerate it)
- the repair would require a broad refactor, public API change, ABI decision, or
  policy decision
- the verification command fails in a way unrelated to the patch
- rerun creates new or noisier cards that the packet did not cover

Stop conditions are a success path. They prevent an agent from turning one
bounded task into an unreviewable rewrite.

## Freshness check before delegating

Packets are snapshots of one review kit. Before handing a packet to an agent:

1. Confirm the `card_id`, file, owner, operation family, and unsafe operation
   still match the current source.
2. Regenerate with `unsafe-review context <card-id> --json` if meaningful
   source changes happened since the first-pr run.
3. Treat a card that disappeared, split, or changed its `missing_evidence` as a
   signal to discard the old packet and start fresh.

## Handoff prompt template

For a `ready_for_agent` card:

```text
Use the attached unsafe-review context packet.
Address card <card-id> only.
Allowed file scope: <explicit paths>.
Add or expose the missing evidence named by the card.
Use only the packet's allowed_repairs.
Copy and obey do_not_do and stop_conditions exactly.
Run only the verify_commands that are available locally.
Return: the patch summary, commands run, and whether the ReviewCard evidence
improved after rerun.
```

Include these boundary lines verbatim:

```text
Do not claim proof, UB-free status, Miri-clean status, site execution,
calibrated precision/recall, or automatic safety repair.
Do not post comments or block a PR.
Do not edit outside the supplied file scope.
```

## Validating the repair

After the agent returns a patch:

1. Apply the patch.
2. Rerun: `unsafe-review first-pr --base origin/main`
3. Run: `unsafe-review outcome --before <before-snapshot> --after target/unsafe-review/cards.json`
4. The card should show `improved` (reclassified to a less-severe class) or
   `resolved` (if the site left scope). `new_gaps=0` and `worsened_gaps=0` are
   also required.
5. If the card did not improve, the repair did not address the named missing
   evidence — do not accept it.

A patch that compiles and passes tests but leaves the card unchanged is not a
valid repair. The correctness target is the card's missing evidence, not
general code style.

## Improved-movement rewards adding evidence, not deleting cards

Agents sometimes try to remove unsafe blocks to eliminate cards. Deletion is not
automatically a repair. It is valid only when the packet explicitly allows
replacing the unsafe operation with a safe API and the behavior is preserved.
Suppressing a card (adding it to the baseline or suppressions file) without
addressing the obligation is never a valid repair.

## Receipts: what agents can and cannot do

If a card is `requires_witness_receipt`, the remaining work is outside source
edits. An agent may:

- Help format a receipt from actual command, output, author, and timestamp data
  supplied by the reviewer.

An agent MUST NOT:

- Fabricate witness execution.
- Write a receipt for a command that was not run.
- Upgrade a `requires_witness_receipt` card to `ready_for_agent` status by
  claiming the receipt was obtained.

Receipts must be imported through:

```bash
unsafe-review receipt template <card-id> \
  --tool <miri|loom|shuttle|asan|sanitizer|human> \
  --strength <ran|reviewed|asserted> \
  --author <name> \
  --recorded-at <iso-timestamp> \
  --expires-at <iso-date> \
  --summary "..." \
  --command "..." \
  --limitation "..." \
  --out .unsafe-review/receipts/<card-id>-<tool>.json
```

Only after the witness or human review happened and the reviewer has the
actual output in hand.

## Further reading

- [Bounded agent repair workflow](agent-repair-workflow.md) — the full reviewer-
  facing workflow with queue triage, freshness checks, and handoff template
- [Agent packet examples](agent-packet-examples.md) — concrete JSON for common
  card families with fixture backing
- [ReviewCard fix recipes](fix-recipes.md) — per-operation-family repair
  guidance for common card shapes
- `docs/specs/UNSAFE-REVIEW-SPEC-0013-agent-packets.md` — the normative spec
  for the context packet schema
