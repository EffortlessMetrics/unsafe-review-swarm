# Agent Packet Examples

`unsafe-review context <card-id> --json` turns one `ReviewCard` into a bounded
repair packet for an LLM or another agent. The packet is copy-only. It does not
run an agent, edit source, run witnesses, post comments, or resolve the card.

These examples show how to read the packet for common card families. They are
fixture-backed examples of packet intent, not proof that the example code is
safe.

For the operational handoff path, see
[Bounded agent repair workflow](agent-repair-workflow.md).

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
    "state": "ready_for_agent"
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
- `confirmation_cue`: the hypothesis, first confirmation/build cue, minimal
  repro recipe, and reminder that receipts or reviewer confirmation are
  required before upgrading confidence.
- `repair_queue`: compact buckets that help sort this one card into guard,
  contract, test, witness, or human-review work.
- `verify_commands`: suggested commands from the card. They are not executed by
  `unsafe-review`.
- `do_not_do`: negative instructions that prevent broad unsafe rewrites,
  suppressing the current card as a repair, broad suppressions, false witness
  claims, automatic safety-repair claims, unrelated edits, and replacing
  executable guard/discharge evidence with comments or docs.
- `stop_conditions`: when the agent should stop and hand the result back.

Readiness states are closed vocabulary:

- `ready_for_agent`: an agent may work the bounded card-scoped repair.
- `requires_human_review`: summarize and hand to a human reviewer before edits.
- `requires_witness_receipt`: run or attach external witness evidence outside
  `unsafe-review`; do not treat this as an edit task.
- `unsupported`: do not delegate as repair work from this packet.

The verifier enforces `ready = true` only for `ready_for_agent`; all other
states must have `ready = false`.

## Repair Queue Examples

`repair-queue.json` aggregates the same card-scoped packet metadata. These
fixture-backed examples show how operation families should land in queue
buckets; the buckets are handoff labels, not proof, source edits, comments, or
witness execution.

| Fixture | Operation family | Non-empty buckets | Agent-ready |
|---|---|---|---|
| `raw_pointer_alignment` | `raw_pointer_read` | `repairable_by_guard`, `requires_witness_receipt` | yes |
| `vec_set_len` | `vec_set_len` | `repairable_by_guard`, `requires_witness_receipt` | yes |
| `str_from_utf8_unchecked` | `str_from_utf8_unchecked` | `repairable_by_guard`, `requires_witness_receipt` | yes |
| `maybeuninit_assume_init` | `maybe_uninit_assume_init` | `repairable_by_guard`, `requires_witness_receipt` | yes |
| `nonnull_other_guard_not_evidence` | `nonnull_unchecked` | `repairable_by_guard`, `requires_witness_receipt` | yes |
| `ffi_sanitizer_route` | `ffi` | `repairable_by_guard`, `repairable_by_test`, `requires_witness_receipt`, `requires_human_review`, `do_not_auto_repair` | no |
| `atomic_pointer_state_swap` | `atomic_pointer_state` | `repairable_by_guard`, `repairable_by_safety_docs`, `requires_witness_receipt`, `requires_human_review`, `do_not_auto_repair` | no |
| `unsafe_impl_send` | `unsafe_impl_send_sync` | `repairable_by_guard`, `requires_witness_receipt`, `requires_human_review`, `do_not_auto_repair` | no |
| `inline_asm_human_review` | `inline_asm` | `repairable_by_guard`, `requires_witness_receipt`, `requires_human_review`, `do_not_auto_repair` | no |
| `split_unsafe_block` | `unknown` | `repairable_by_guard`, `repairable_by_safety_docs`, `repairable_by_test`, `requires_witness_receipt`, `requires_human_review`, `do_not_auto_repair` | no |

## Raw Pointer Alignment

Fixture proof:

- `raw_pointer_alignment`

Packet focus:

```text
operation_family: raw_pointer_read
missing: alignment / pointer / initialization / allocation evidence as reported by the card
agent_readiness: ready_for_agent
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
agent_readiness: ready_for_agent when the packet has scoped repairs and verify commands
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
agent_readiness: ready_for_agent when the unsafe site and validation target are specific
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
agent_readiness: ready_for_agent when the card identifies one concrete pointer
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
  suppress this card instead of exposing same-pointer evidence
  widen the unsafe block
```

Why this is bounded:

- The packet ties allowed repairs to `nonnull_unchecked`, not to all pointer
  operations in the file.
- The packet can be ready for a small repair, but the trust boundary still says
  this is static unsafe contract review only.

## Bounds-Checked Indexed Access

Fixture proof:

- `get_unchecked_mut_get_probe_guard`
- `get_unchecked_mut_get_probe_reassigned_index_not_guard`
- `get_unchecked_mut_other_len_not_guard`

Packet focus:

```text
operation_family: get_unchecked
missing: same-slice / same-index bounds evidence or witness evidence
agent_readiness: ready_for_agent only when the packet has card-scoped same-index repairs
```

Useful handoff:

```text
Task:
  Add visible bounds evidence for this exact get_unchecked call.

Allowed:
  add a same-slice get(index) probe that returns or errors on None
  add a same-slice length/range guard before the unchecked access
  preserve the same index value between the guard and unchecked access

Do not:
  check a different slice
  check a different or reassigned index
  treat a post-access check as evidence
  use a comment-only return as executable guard evidence
```

Why this is bounded:

- The packet should keep the repair scoped to one slice/index pair.
- If the packet cannot identify the same slice and index, the safe next action is
  reviewer clarification rather than automatic repair delegation.

## `MaybeUninit::assume_init` Initialization

Fixture proof:

- `maybeuninit_assume_init_write_guard`
- `maybeuninit_assume_init_open_branch_write_guard`
- `maybeuninit_assume_init_stale_write_not_guard`
- `maybeuninit_assume_init_stale_field_write_not_guard`
- `maybeuninit_assume_init_comment_not_guard`

Packet focus:

```text
operation_family: maybe_uninit_assume_init
missing: same-slot initialization evidence or witness evidence
agent_readiness: ready_for_agent only when the initialized slot and unsafe site are specific
```

Useful handoff:

```text
Task:
  Add or expose initialization evidence for this exact assume_init site.

Allowed:
  write or construct the same MaybeUninit slot before assume_init
  keep the initialization branch open to the unsafe site
  replace assume_init with a checked construction path when API intent allows it

Do not:
  write a different slot
  reassign the slot after initialization
  rely on a closed observation branch
  add only a SAFETY comment
```

Why this is bounded:

- Initialization evidence is slot-specific. A nearby write is useful only if it
  is for the same value that reaches `assume_init`.
- Related test mentions can justify a witness route, but they are not proof that
  the unsafe site executed.

## `Vec::set_len` Initialized Range

Fixture proof:

- `vec_set_len_initialized_loop`
- `vec_set_len_reserve_capacity`
- `vec_set_len_cap_argument_not_guard`
- `vec_set_len_reassigned_receiver_not_guard`

Packet focus:

```text
operation_family: vec_set_len
missing: same-vector capacity evidence, initialized-range evidence, or witness evidence
agent_readiness: ready_for_agent when the packet separates capacity from initialized length
```

Useful handoff:

```text
Task:
  Make the set_len preconditions visible for this exact vector and length.

Allowed:
  add a same-vector capacity guard for the requested length
  initialize every newly exposed element before set_len
  keep receiver and length values fresh between guard/init and set_len

Do not:
  treat spare capacity as initialized elements
  check a capacity argument that is not the set_len target
  rely on initialization of an unrelated vector
  move the guard after set_len
```

Why this is bounded:

- `Vec::set_len` has two different review questions: capacity and
  initialization. The packet should not collapse them into one generic length
  check.
- Capacity evidence can reduce one obligation while initialized-range evidence
  remains missing.

## `transmute` Value Domain

Fixture proof:

- `transmute_bool_valid_value_guard`
- `transmute_bool_guard_then_reassigned_not_guard`
- `transmute_layout_size_guard`
- `transmute_copy_bool_valid_value_guard`

Packet focus:

```text
operation_family: transmute
missing: layout evidence, valid-value evidence, or witness evidence
agent_readiness: ready_for_agent only for narrow value-domain repairs
```

Useful handoff:

```text
Task:
  Add value-domain or layout evidence for this exact transmute.

Allowed:
  replace the transmute with a checked conversion when available
  add an open same-value guard for the valid target-domain values
  keep layout evidence separate from valid-value evidence

Do not:
  treat size equality as proof that every bit pattern is valid
  guard one value and transmute a reassigned value
  use a comment-only assertion as executable evidence
  generalize a bool-domain guard to other target types
```

Why this is bounded:

- Some transmute obligations are syntactic layout checks; others are semantic
  valid-value checks. The packet should name which obligation is missing.
- When the value domain is not statically knowable, the packet should route to
  human review instead of suggesting a broad rewrite.

## Atomic Pointer State

Fixture proof:

- `atomic_pointer_state_fetch_ops`
- `atomic_pointer_state_swap`

Packet focus:

```text
operation_family: atomic_pointer_state
missing: concurrency/state invariant evidence or specialist witness route
agent_readiness: usually requires_human_review
```

Useful handoff:

```text
Task:
  Summarize the atomic pointer invariant and the missing evidence for review.

Allowed:
  document ownership, lifetime, ordering, and reclamation invariants
  suggest Loom or Shuttle only when the card already routes there
  ask for a focused human review of the state machine

Do not:
  claim concurrency proof from a normal unit test
  add ordering changes speculatively
  treat a successful compare/swap as lifetime proof
  ask an agent to rewrite the state machine automatically
```

Why this is not normally a repair-ready packet:

- Atomic pointer state depends on interleavings, ownership transfer, and memory
  reclamation rules that are rarely proven by local syntax alone.
- The packet is useful as a bounded reviewer brief, but it should not invite
  automatic concurrency repair.

## FFI Boundary

Fixture proof:

- `ffi_sanitizer_route`

Packet focus:

```text
operation_family: ffi
agent_readiness: requires_human_review
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
agent_readiness: requires_human_review
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

## Human Review Only Or Unknown Family

Packet focus:

```text
operation_family: unknown or unsupported
agent_readiness: requires_human_review
```

Useful handoff:

```text
Task:
  Summarize the card and the missing evidence for a human reviewer.

Allowed:
  add missing contract text when the ReviewCard asks for contract evidence
  split the unsafe boundary into a smaller review if the card is too broad
  collect a focused witness only when the card names a concrete route

Do not:
  ask an agent to rewrite the unsafe code broadly
  invent a guard that is not tied to the ReviewCard obligation
  suppress this card as a repair instead of documenting an explicit waiver with
  owner, expiry, and evidence
  claim unsupported static evidence proves safety
```

Why this is not a repair-ready packet:

- Unknown and unsupported cards are still useful review work, but the packet
  should not promote them into automatic source edits.
- The safe handoff is a bounded human-review brief: what changed, why it matters,
  what evidence is missing, and which route could add signal.

## Review Checklist

Before handing a packet to an agent, check:

- The packet is for one `card_id`.
- `agent_readiness.state` is `ready_for_agent` before delegating repair work.
- `allowed_repairs` matches the missing obligation.
- `source_context` is enough to orient the task without dumping whole files.
- `verify_commands` are suggestions only; a receipt is needed before claiming a
  witness result.
- The `do_not_do` and `stop_conditions` are copied with the task.
- The task does not replace executable guard or discharge evidence with comments
  or docs.
- The task does not claim proof, UB-free status, Miri-clean status, site
  execution, calibrated precision/recall, source edits by `unsafe-review`, or
  automatic comment posting.

The conservative default is to ask the agent to stop after one card and return a
patch, validation output, and any remaining missing evidence.
