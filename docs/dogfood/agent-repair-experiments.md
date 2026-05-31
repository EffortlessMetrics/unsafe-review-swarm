# Agent Repair Experiment Protocol

Status: experimental dogfood protocol

This protocol records whether a card-scoped agent handoff helped produce a
bounded, reviewable repair attempt. It measures the usefulness of
`unsafe-review context <card-id> --json` and `repair-queue.json` as inputs to a
human-run experiment. It does not add automation and does not make a product
claim.

The experiment question is:

```text
Can an agent use only one ReviewCard packet, one repair-queue item, and an
explicit file scope to produce a narrow patch that a maintainer can review?
```

## Inputs

Record these inputs before starting:

| Field | Meaning |
|---|---|
| `experiment_id` | Stable local ID, such as `arrayvec-pr288-set-len-guard-001`. |
| `target` | Dogfood target or fixture name. |
| `report` | Source dogfood report or fixture note. |
| `card_id` | The ReviewCard being handed to the agent. |
| `operation_family` | Operation family from the ReviewCard. |
| `context_command` | Exact `unsafe-review context <card-id> --json` command. |
| `repair_queue_item` | Bucket and reason from `repair-queue.json`. |
| `allowed_scope` | Exact files or fixture directory the experiment may touch. |
| `baseline_artifacts` | Card or review-kit artifacts used before the patch. |

Use checked-in fixtures for the first experiments. Move to real dogfood clones
only after the fixture dry run stays card-scoped and reviewable.

## Agent Instruction

The agent instruction must be card-sized:

```text
Use the attached context packet and repair-queue item.
Address this one ReviewCard only.
Stay inside the listed file scope.
Add or expose the specific missing evidence named by the card.
Return a patch summary and validation commands.
```

The instruction must also include these boundaries:

```text
Do not suppress this card as the repair.
Do not replace executable guard or discharge evidence with a comment.
Do not edit unrelated unsafe sites.
Do not claim proof, UB-free status, Miri-clean status, site execution, witness
execution, witness adequacy, calibrated precision/recall, or policy readiness.
Stop if the packet points to human deep review, unsupported provenance, broad
FFI ownership review, macro/cfg ambiguity, or a repair outside the listed scope.
```

## Human Review Record

After the dry run, record:

| Field | Meaning |
|---|---|
| `patch_summary` | What the agent changed, in one or two sentences. |
| `validation` | Commands run by the human or agent. |
| `card_delta` | Whether the target ReviewCard improved, stayed the same, or regressed. |
| `scope_delta` | Whether the patch stayed inside the allowed scope. |
| `new_cards` | Whether the patch introduced additional ReviewCards. |
| `reviewer_judgment` | `good-agent-task`, `bad-agent-task`, `human-only`, or `uncertain`. |
| `reason` | Why the packet was or was not useful for delegation. |
| `follow_up` | One next PR seed, fixture, docs note, or no follow-up. |

Use `good-agent-task` only when the packet produced a bounded patch that a human
can review against the original card. Use `bad-agent-task` when the packet was
underconstrained, encouraged unrelated edits, or failed to name the missing
evidence clearly enough. Use `human-only` when the useful action is contract or
design review rather than a patch. Use `uncertain` when the experiment did not
exercise enough context to decide.

## Minimal Template

```toml
schema_version = "0.1"
experiment_id = "fixture-utf8-same-buffer-001"
target = "str_from_utf8_unchecked"
report = "fixtures/str_from_utf8_unchecked/expected.cards.json"
reviewer = "manual"
date = "2026-05-31"
card_id = "UR-..."
operation_family = "str_from_utf8_unchecked"
context_command = "unsafe-review context UR-... --json"
repair_queue_bucket = "repairable_by_guard"
allowed_scope = ["fixtures/str_from_utf8_unchecked/"]
baseline_artifacts = ["fixtures/str_from_utf8_unchecked/expected.cards.json"]

agent_instruction = "Resolve this one card by adding or exposing same-buffer UTF-8 validation. Do not suppress the card or edit unrelated unsafe sites."
patch_summary = "The dry run added a same-buffer validation guard before the unchecked conversion."
validation = ["cargo test -p unsafe-review-core utf8"]
card_delta = "improved"
scope_delta = "inside_allowed_scope"
new_cards = "none_observed"
reviewer_judgment = "good-agent-task"
reason = "The packet named one missing same-buffer evidence shape and the patch stayed card-scoped."
follow_up = "Record as a fixture dry run before using a real dogfood clone."
trust_boundary = "Static unsafe contract review experiment; not calibrated precision or recall, not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site execution evidence, not witness adequacy, and not policy readiness. unsafe-review did not run an agent, execute witnesses, post comments, edit source, suppress cards, or enforce blocking policy."
```

## Trust Boundary

Agent repair experiments are manual dogfood measurements of handoff usefulness.
They are not calibrated precision or recall, not a proof of memory safety, not
UB-free status, not a Miri result, not Miri-clean status, not site execution
evidence, not witness adequacy, not release readiness, and not policy
readiness. `unsafe-review` does not run agents, execute witnesses, post
comments, edit source, suppress cards, resolve cards, or enforce blocking
policy.
