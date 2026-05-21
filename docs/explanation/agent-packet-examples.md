# Agent Packet Examples

`unsafe-review context <card-id> --json` turns one `ReviewCard` into a bounded
repair packet for an LLM or another agent. The packet is copy-only. It does not
run an agent, edit source, run witnesses, post comments, or resolve the card.

These examples show how to read the packet for common card families. They are
fixture-backed examples of packet intent, not proof that the example code is
safe.

## Packet Shape

Every packet projects from one `ReviewCard` and keeps the same trust boundary:

```json
{
  "mode": "bounded_repair_packet",
  "source": "review_card",
  "policy": "advisory",
  "repair_scope": "this card only",
  "agent_readiness": {
    "ready": true,
    "state": "ready"
  }
}
```

The useful fields for an agent handoff are:

- `task`: the reviewer-facing next action from the card.
- `source_context`: a bounded context window with the unsafe site, nearby
  contract/guard summaries, and a few related test mentions.
- `missing_evidence`: the exact missing evidence the repair must address.
- `allowed_repairs`: card-scoped repairs derived from operation family and
  missing obligations.
- `verify_commands`: suggested commands from the card. They are not executed by
  `unsafe-review`.
- `do_not_do`: negative instructions that prevent broad unsafe rewrites,
  suppressions, false witness claims, and unrelated edits.
- `stop_conditions`: when the agent should stop and hand the result back.

## Raw Pointer Alignment

Fixture proof:

- `raw_pointer_alignment`

Packet focus:

```text
operation_family: raw_pointer_read
missing: alignment / pointer / initialization / allocation evidence as reported by the card
agent_readiness: ready
```

Useful handoff:

```text
Task:
  Resolve this one raw-pointer read card.

Allowed:
  add a same-pointer alignment guard before the read
  or switch to an unaligned operation only if unaligned input is intended
  attach a scoped witness receipt after running the suggested command externally

Do not:
  add only a SAFETY comment
  check a different pointer
  claim Miri proof without a receipt
  edit unrelated unsafe code
```

Why this is bounded:

- `source_context.unsafe_site` names only the unsafe read site.
- `source_context.nearby_guard_evidence` can show that bounds evidence exists,
  but it does not treat that as alignment evidence.
- Related test mentions remain reach hints, not site-execution proof.

## `copy_nonoverlapping` Range

Fixture proof:

- `copy_nonoverlapping`
- `copy_nonoverlapping_slice_range_guard`

Packet focus:

```text
operation_family: copy_nonoverlapping
missing: source range, destination range, non-overlap, or witness evidence
agent_readiness: ready when the packet has scoped repairs and verify commands
```

Useful handoff:

```text
Task:
  Add visible range evidence for this exact copy call.

Allowed:
  prove count fits the source range used by the call
  prove count fits the destination range used by the call
  prove source and destination ranges do not overlap

Do not:
  check only src.len()
  check only dst.len()
  leave the early return only in a comment
  reassign src, dst, or count after the guard
```

Why this is bounded:

- `allowed_repairs` should name range and non-overlap evidence, not unrelated
  pointer alignment work.
- A witness command can be copied, but running it and recording a receipt is
  outside the packet.

## UTF-8 Unchecked Conversion

Fixture proof:

- `str_from_utf8_unchecked`
- `str_from_utf8_unchecked_is_ok_guard`
- `str_from_utf8_unchecked_is_err_return_guard`

Packet focus:

```text
operation_family: str_from_utf8_unchecked
missing: same-buffer UTF-8 validation or witness evidence
agent_readiness: ready when the unsafe site and validation target are specific
```

Useful handoff:

```text
Task:
  Add validation evidence for the same byte buffer before from_utf8_unchecked.

Allowed:
  add an early return on from_utf8(bytes).is_err()
  use from_utf8(bytes)? where the surrounding API can return an error
  replace the unchecked conversion with a checked conversion if that preserves API intent

Do not:
  validate another buffer
  validate after the unchecked conversion
  rely on a closed observation branch
  reassign bytes after validation
```

Why this is bounded:

- The packet names the operation family and unsafe site, so the agent should not
  rewrite unrelated string parsing.
- The missing evidence is obligation-specific: same-buffer validation, not a
  generic comment about UTF-8.

## `NonNull::new_unchecked` Nullability

Fixture proof:

- `nonnull_new_guard`
- false-positive controls such as `nonnull_other_guard_not_evidence`

Packet focus:

```text
operation_family: nonnull_unchecked
missing: same-pointer non-null evidence or witness evidence
agent_readiness: ready when the card identifies one concrete pointer
```

Useful handoff:

```text
Task:
  Resolve this one NonNull::new_unchecked card.

Allowed:
  use NonNull::new(ptr) when the caller can handle None
  add a same-pointer non-null guard before new_unchecked
  attach a scoped witness receipt after running the suggested command externally

Do not:
  check a different pointer
  check for null after new_unchecked
  add a broad suppression
  widen the unsafe block
```

Why this is bounded:

- The packet ties allowed repairs to `nonnull_unchecked`, not to all pointer
  operations in the file.
- The packet can be ready for a small repair, but the trust boundary still says
  this is static unsafe contract review only.

## FFI Boundary

Fixture proof:

- `ffi_sanitizer_route`

Packet focus:

```text
operation_family: ffi
agent_readiness: not ready / needs human review
```

Useful handoff:

```text
Task:
  Prepare a human-review note for the FFI boundary.

Allowed:
  document ABI, ownership, lifetime, mutation, and free responsibilities
  suggest sanitizer or cargo-careful receipt evidence after external execution

Do not:
  ask an agent to rewrite the foreign boundary automatically
  claim Miri coverage for foreign implementation behavior
  treat the Rust declaration as proof of the foreign contract
```

Why this is not a repair-ready packet:

- The boundary depends on foreign code and ownership contracts that may not be
  visible in local Rust syntax.
- `agent_readiness.reasons` should explain why the packet routes to human review
  or external witness work instead of automatic repair delegation.

## Inline Assembly Or Target Features

Fixture proof:

- `inline_asm_human_review`
- `target_feature_safety_docs`

Packet focus:

```text
operation_family: inline_asm or target_feature
agent_readiness: not ready / needs human review
```

Useful handoff:

```text
Task:
  Summarize the safety contract and missing evidence for a human reviewer.

Allowed:
  document architecture assumptions
  document register, memory, and target-feature preconditions
  suggest a focused witness route when one is already present in the card

Do not:
  let an agent rewrite assembly constraints speculatively
  promote target-feature documentation to runtime proof
  claim safety from a test mention
```

Why this is not a repair-ready packet:

- The packet is still useful as a summary, but the risk is not narrow enough for
  automatic source edits.
- `agent_readiness` should prevent treating this as a normal repair task.

## Review Checklist

Before handing a packet to an agent, check:

- The packet is for one `card_id`.
- `agent_readiness.ready` is true for repair work, or the task is explicitly
  human-review-only.
- `allowed_repairs` matches the missing obligation.
- `source_context` is enough to orient the task without dumping whole files.
- `verify_commands` are suggestions only; a receipt is needed before claiming a
  witness result.
- The `do_not_do` and `stop_conditions` are copied with the task.

The conservative default is to ask the agent to stop after one card and return a
patch, validation output, and any remaining missing evidence.
