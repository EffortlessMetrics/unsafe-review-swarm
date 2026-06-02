# UNSAFE-REVIEW-SPEC-0008: Witness routing

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md

## Problem

`unsafe-review` needs a precise, checkable behavior contract for witness routing.

## Behavior

Route hazards to Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, Crux,
or human deep review.

ReviewCards carry witness routes as advisory next steps. A route must name the
route kind and rationale. If a concrete command is available, the same command
must appear in `verify_commands`; if `verify_commands` names a command, that
command must come from a witness route. Routes are never required by default:
they identify the cheapest credible next witness, not proof that the witness
has run.

FFI ABI and ownership hazards route to sanitizer, cargo-careful, and
human-deep-review. The human-deep-review route is for seams where a reviewer has
checked the Rust extern declaration against the cited foreign declaration or
ownership contract and executable witnesses cannot cross that boundary. It has
no verify command and is still only receipt metadata until a matching exact-card
receipt is imported.

## Non-goals

- no soundness claim
- no hidden blocking unless policy mode explicitly enables it
- no duplicate truth outside this spec and linked policy files

## Required evidence

- fixture-backed examples for positive and negative cases
- JSON output contract coverage
- human output smoke coverage
- policy documentation when behavior is configurable

## Acceptance examples

- A changed unsafe seam produces one review card with stable identity.
- The card includes missing evidence and a next action.
- If evidence is not knowable statically, the card names the limitation instead of overclaiming.
- A card with a Miri route command exposes that command in `verify_commands`.
- A human deep review route may have no command and therefore no verify command.
- An FFI card includes a human deep-review route in addition to executable
  sanitizer/cargo-careful routes.
- A fixture cannot mark a witness route as required by default.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
