# Product principles

This document states the core design principles of `unsafe-review` as settled
doctrine. It collects the product stances that recur across specs, ADRs, and
improvement sessions — encoding them once so they can be referenced without
repetition.

## Trust boundary

Everything in this document is a methodology and design record. It contains no
memory-safety proof, no UB-free status, no Miri-clean status, no
calibrated precision/recall figures, and no site-execution evidence.
The principles describe advisory static-review design intent.

---

## Restraint is the design value

The principles below share a common shape: they describe what the tool
deliberately does not do, and why. Each restraint is a decision, not a
limitation. The product is trustworthy because it holds its restraints under
pressure — when a card feels noisy, when a surface could claim more, when a
detector could fire more broadly. Restraint held under pressure is the product.

---

## Semantic-light / build-free by choice: reach over resolution

`unsafe-review` analyzes syntax, tokens, and diff hunks without requiring a
successful build of the target repo. It does not perform target-repo type
resolution, trait inference, MIR analysis, or macro expansion at scan time.
This is a deliberate design choice, not an interim limitation.

The tradeoff: reach over resolution. The tool runs on broken PRs, huge monorepos
in fast CI lanes, and repos with network-dependent deps. It works because it does
not require the build to succeed. The cost is that every detector must explicitly
earn the structural properties (scope, call-vs-definition, binding identity, span
type) that a type-aware analyzer would get for free from resolved bindings — the
D1–D5 discipline tax documented in SPEC-0005.

When the discipline is paid, the detector is correct. The fix for a false positive
is to add the discipline, not to add type resolution.

---

## Advisory by design: the tool builds gates to stop itself overclaiming

`unsafe-review` is advisory. It names obligations and gaps; it does not render
verdicts. This is not hedging — it is the product contract. The tool is used in
contexts where a false "UB-free" claim would be worse than no claim at all, so
the tool is architecturally prevented from making one.

The trust-boundary gates (claim-boundary agent, xtask wording checks, e2e wording
tests) exist to stop the tool overclaiming across every output surface. Advisory
is enforced, not aspirational.

Concretely: no surface may claim memory-safety proof, UB-free status, Miri-clean
status (without a matching witness receipt), site execution, calibrated
precision/recall, default blocking, or automatic comment posting. Every output
surface must be projectable from the ReviewCard without adding claims the card
does not support.

---

## Evidence not proof: a failing witness raises urgency, never clears the obligation

The evidence a card records is evidence — not proof. A `SAFETY` comment names a
contract obligation; it is not a guard. A test that reaches the unsafe site is
evidence of reach; it is not a site-execution receipt. A witness receipt records
that a specific tool ran and completed; it is not a UB-free certificate.

Corollary: a failing witness (Miri exit non-zero, Loom finds a schedule, Kani
finds a counterexample) raises urgency on the card and should be surfaced. It
does not clear the obligation — it intensifies it. The evidence model is
monotonic: more evidence can only strengthen or refine a card, never discharge it
by proxy.

---

## Surfaces project, never re-derive: single truth

The ReviewCard is the single truth object. Every output surface — CLI output,
JSON, PR summary, SARIF, LSP diagnostics, agent packets, badges, baselines,
suppressions, witness receipts — is a projection of the card. No surface
re-derives a value the pipeline already computed.

This principle is violated when a surface computes `unsafe_sites = cards.len()`
instead of reading the sites field, or when two surfaces independently compute
movement status and disagree. The fix is always to identify the canonical
derivation and project from it, not to reconcile two independent derivations.

A second truth surface is a bug waiting to diverge.

---

## Group and rank, never delete: evidence integrity

When a correct card feels noisy, the answer is to group, rank, and budget the
surfacing — not to delete the card. The evidence layer (`ReviewCard`) must be
complete. Every card that reflects a genuine obligation gap belongs there.

The surfacing layer (comment-plan, PR-summary budget, LSP packet, ub-review
group/rank pass) applies audience-specific selection and ranking. Suppressing
evidence to reduce perceived noise erodes the instrument; grouping and ranking
reduce cognitive load without loss of information.

"Rigor-vs-noise is a presentation problem, not a detection one."

---

## Naming is not doing

A `SAFETY` comment names an obligation but is not a guard. A `use` statement
names a function but is not a call. A test fixture that `stringify!`s a function
name or imports it names the function but does not call it — so it does not
establish test reach.

This principle is the product's canonical error: the most common unsafe-contract
gap is a codebase that names its safety obligations without discharging them. The
tool holds its own test suite and detectors to the same standard.

Call-shaped evidence is required. Naming is not doing.

---

## The detector-discipline control plane: the analyzer holds itself to its own standard

The SPEC-0005 appendix defines D1–D5, the five discipline checks every detector
must apply: scope gate, definition-vs-call gate, same-origin discharge,
string/comment masking, path-segment anchoring.

This is not a technical checklist. It is the analyzer holding itself to the same
standard it holds users' code:

- Users' code must declare its safety obligation (SAFETY comment) and discharge
  it with evidence (guard, test, witness receipt).
- Each detector must declare its obligations (which disciplines it applies) and
  discharge them with evidence (fixture-encoded negative controls per discipline).

The SPEC-0005 appendix is the detector's own SAFETY comment. A detector that
fires without a scope check has the same structure as code with a SAFETY comment
but no guard: it named the obligation without discharging it.

The symmetry is the principle.

---

## Cross-references

- SPEC-0028 surfacing-is-not-suppression and rigor-is-the-product:
  `docs/specs/UNSAFE-REVIEW-SPEC-0028-delivery-surfaces-and-ease-of-use.md`
- Detector discipline D1–D5:
  `docs/specs/appendices/UNSAFE-REVIEW-SPEC-0005-appendix-operation-family-registry.md`
- Analyzer improvement learnings and second-arc synthesis:
  `docs/contributing/ANALYZER-LEARNINGS.md`
- ADR-0002 ReviewCard as canonical:
  `docs/adr/UNSAFE-REVIEW-ADR-0002-review-card-is-canonical.md`
- ADR-0005 advisory-first policy:
  `docs/adr/UNSAFE-REVIEW-ADR-0005-advisory-first-policy.md`
- ADR-0009 syntax-first detection:
  `docs/adr/UNSAFE-REVIEW-ADR-0009-syntax-first-detection.md`
