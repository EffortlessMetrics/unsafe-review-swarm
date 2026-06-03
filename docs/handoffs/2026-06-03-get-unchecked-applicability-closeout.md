# 2026-06-03 get_unchecked applicability closeout

Status: swarm handoff
Scope: `get_unchecked` / `get_unchecked_mut` bounds applicability rails

This closeout turns the recent `get_unchecked` applicability work into a
maintained capability note. It is not a new analyzer change, support-tier
promotion, calibration report, source promotion, or release note.

Trust boundary:

- static advisory review only
- no witness execution by default
- no automatic comments
- no source edits
- no default blocking
- no proof, safety, UB-free, Miri-clean, site-execution, or calibrated
  precision/recall claim

## What Is Now Pinned

The current fixture label ledger
`docs/accuracy/labels/get-unchecked-mut-bounds.toml` has 49 samples for the
`get_unchecked` operation family and `bounds` obligation:

- 8 positive samples: one bare missing-bounds smoke case plus accepted local
  evidence shapes.
- 41 false-positive controls for wrong-target, stale-target, dominance, and
  prose-only cases.

The accepted evidence shapes are fixture-pinned only:

- bare `get_unchecked_mut` without bounds evidence remains `guard_missing`
- same-receiver `index < receiver.len()` / `receiver.len() > index`
- top-level conjunctive len guards while the unsafe access remains in the open
  branch
- same-receiver `get(index).is_some()` branch containing the unsafe access
- same-receiver `get(index).is_none()` early return before the unsafe access
- same-receiver `if let Some(..) = receiver.get(index)` guard
- same-receiver `let Some(..) = receiver.get(index) else { return .. }` guard
- same-receiver `match receiver.get(index)` `Some` arm containing the unsafe
  access

The implementation checkpoint is recorded in
`docs/analysis/evidence-applicability-model.md`: the family uses
`GetUncheckedBoundsApplicability` for same receiver/index, top-level
conjunctive open branches, early returns, and stale target rejection.

## False Positives Controlled

The current controls reject evidence when it does not answer the exact bounds
question for the exact unsafe access:

- other slice or other receiver len/probe evidence
- disjunctive bounds branches that can reach the unsafe operation with the
  bounds predicate false
- post-checks after the unsafe operation
- observed-only bounds predicates and closed positive branches
- comment-only return claims
- reassigned, compound-mutated, or shadowed index values
- reassigned receiver bindings
- reassigned or shadowed receiver paths
- stale index, receiver, or receiver-path evidence inside direct get-probe,
  early-return get-probe, if-let, let-else, and match forms

The useful product rule this pins is:

```text
same receiver + same index + dominating executable guard + fresh target
```

Anything weaker stays a review gap.

## Still Not Claimed

This slice still does not claim:

- arbitrary `get_unchecked` soundness
- iterator, aliasing, provenance, or caller-behavior proof
- macro, cfg, or wrapper equivalence
- cross-function invariant proof
- Miri-clean status, UB-free status, or site execution
- calibrated precision/recall
- policy readiness or default blocking suitability

The witness routes remain review guidance only. Suggested `miri` or
`cargo-careful` routes do not become witness evidence unless an external receipt
is attached and audited.

## Fixture-Only Cases

All 49 label samples in `get-unchecked-mut-bounds.toml` are fixture-only. They
are suitable for regression protection and fix-recipe examples, but not for a
support-tier promotion by themselves.

The support-tier row for the stale/wrong-target applicability slice remains
experimental and explicitly limited to fixture-pinned source patterns.

## Dogfood-Observed Cases

No checked-in dogfood report currently proves a `get_unchecked` applicability
outcome change from this burst.

Relevant dogfood context exists:

- `arrayvec-pr137` is a soundness-fix target involving replacement of
  `get_unchecked_mut`-derived references with raw pointer accessors, but the
  checked report records raw-pointer and `Vec::set_len` review posture rather
  than a `get_unchecked` applicability improvement.
- Hashbrown targets remain useful future pressure, but current checked-in
  reports do not establish a `get_unchecked` stale/wrong-target outcome.

Treat future `get_unchecked` analyzer work as dogfood-driven only when a report
records an actual noisy, missed, or weak `get_unchecked` ReviewCard.

## Fix-Recipe Mapping

The `get_unchecked` fix recipe should use this closeout as its evidence rail.

What unsafe-review is looking for:

- same slice or receiver as the unsafe call
- same index value as the unsafe call
- executable bounds guard that dominates the unsafe operation
- guard still fresh after reassignment, mutation, or shadowing checks
- no receiver-path drift between guard and unsafe operation

Good repairs:

- add or move an executable bounds check immediately before the unsafe access
- keep the check on the same receiver and same index
- prefer a branch or early return that makes the unsafe path visibly guarded
- rerun `unsafe-review` and compare whether the bounds obligation moved from
  missing to present

Bad repairs:

- comment-only safety explanations
- checking a different slice, receiver, receiver path, or index
- checking before reassigning or shadowing the index or receiver
- post-checking after the unsafe access
- replacing the local guard with an unaudited cross-function assumption

Witness route:

- focused unit or property test for the safe caller/input that reaches the card
- `miri` or `cargo-careful` when aliasing, provenance, or layout behavior is the
  reviewer question
- receipt metadata only after the witness is run outside `unsafe-review`

What this does not prove:

- the call is sound for all callers
- aliasing/provenance constraints are satisfied
- the code is UB-free, Miri-clean, or policy-ready

## Source Promotion Recommendation

Promote this closeout to source with the broader usability-docs batch only after
the public fix recipes and agent repair workflow exist there too. By itself this
handoff is an internal capability closeout; it becomes useful source-facing
context when linked from the `get_unchecked` recipe and the find/fix workflow.

Do not flatten swarm history during promotion. Use the documented
history-preserving source/sync process, and keep the support posture
experimental unless dogfood evidence later justifies a narrower claim.

## Closeout Decision

Stop adding `get_unchecked` micro-rails unless one of these is true:

- a dogfood usefulness judgment identifies a concrete noisy, missed, or weak
  `get_unchecked` card
- the fix recipe needs one missing fixture to make a user-facing repair rule
  honest
- a source-promotion review finds the handoff inconsistent with current source
  behavior

Otherwise, spend the next work on workflow, recipes, agent repair boundaries,
CI cookbook, and usefulness judgments.

## Validation

Local validation for this closeout:

```bash
rtk cargo run --locked -p xtask -- check-docs
rtk cargo run --locked -p xtask -- check-doc-artifacts
rtk cargo run --locked -p xtask -- check-docs-automation
rtk cargo run --locked -p xtask -- check-pr
```
