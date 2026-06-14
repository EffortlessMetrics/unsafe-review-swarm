# Analyzer improvement: lessons from the card-correctness session

Date: 2026-06-14
Session scope: ~30 card-correctness fixes (#1672–#1707)
Status: methodology record

## Trust boundary

Everything in this document is a methodology record from a static source-text
improvement session. It contains no memory-safety proof, no UB-free status, no
Miri-clean status, no calibrated precision/recall figures, and no site-execution
evidence. It is a tooling lesson, not a coverage or correctness claim.

---

## The session in one sentence

Thirty-odd card-correctness findings across a fixture correctness pass and
fresh-crate dogfood collapsed to five root mechanisms once the detector code was
audited with the real-world cases in hand.

---

## The bug class: the type-free tax on fallback detection

`unsafe-review`'s detection pipeline has two layers:

1. **Syntax path** — AST-backed via `ra_ap_syntax`, which carries scope
   information, node type, binding identity, and call-vs-definition for free.
2. **Fallback text path** — substring matching on compacted whitespace strings,
   which carries none of those properties.

The syntax path is accurate by construction for the cases it handles. Almost every
false positive in the session traced to the fallback path, because text matching
lacks the four properties the syntax tree encodes for free:

- **Scope** — is this inside an unsafe block or unsafe fn body?
- **Node type** — is this a call expression or a function definition header?
- **Binding identity** — is the guard on the same receiver/pointer/slot as the
  operation being discharged?
- **Span type** — is this in a comment or string literal?

When those properties are not explicitly checked in the fallback path, a detector
fires on safe-context code, on `fn foo(` headers, on guards for a different
binding, and on code inside comments. The five discipline checks (D1–D5 in the
SPEC-0005 appendix) encode those properties as explicit gates.

This is not a unique failure mode: any heuristic that operates on raw text without
structural context is subject to the same class of false positive. The lesson is
to centralize those checks at dispatch rather than relying on each detector to
remember them independently.

---

## The methodology: black-box dogfood + white-box audit pairing

The session used two investigation modes in sequence:

**Black-box dogfood first.** Run the tool on fresh, unseen real crates. Note which
cards look wrong — either firing where they should not (false positive) or missing
where they should (false negative). This produces a symptom list, not a root cause.

**White-box audit second.** With each symptom case in hand, read the relevant
detector. Ask: what code path produced this output? Is this a shared mechanism
across multiple detectors, or a one-off? What is the minimal discipline check that
would prevent this class of case?

The pairing matters. Black-box alone produces a long instance list that tempts
instance-by-instance patching. White-box alone produces abstract code analysis
without real-world triggers to validate against. Together they converge on the
mechanism.

---

## Collapse-to-root: fix the mechanism, not the instances

Once the mechanism is identified, the correct repair is at the mechanism level,
not the instance level.

In this session: 28+ false-positive instances across the fixture suite and dogfood
targets collapsed to five root mechanisms once the detector code was read with the
real cases in hand:

1. **D1: Unsafe-scope gate** — detectors firing on safe-context code (no enclosing
   `unsafe { }`, not inside `unsafe fn`). Fixed by adding a scope check at the
   point where the text path is entered.
2. **D2: Definition-vs-call gate** — detectors firing on `fn NAME(` function
   definition headers rather than call sites. Fixed by rejecting the pattern on
   definition-looking lines.
3. **D3: Same-origin discharge** — evidence admitted for a guard on a different
   binding (different pointer, different slice, different slot) than the one the
   operation uses. Fixed by requiring binding identity before accepting discharge
   evidence.
4. **D4: String/comment masking** — detectors firing on text inside string literals
   or doc comments. The syntax path handles this by construction; the text fallback
   must verify span type.
5. **D5: Path-segment anchoring** — a call path whose tail segment spells a module
   name (e.g. `registry_ptr::read_entry`) being matched as `std::ptr`. Fixed by
   verifying the module prefix at the correct segment position.

Each mechanism fix cleared multiple instances simultaneously. The resulting
negative-control fixtures (one per discipline per family where applicable) encode
the regression guard so the class cannot silently regrow.

---

## Fixture-suite blindness

The most important structural lesson: the fixture suite encodes what the author
knew to test for. It is blind to what the author did not know to test.

Evidence: the wave-1 fixtures for every detector placed the operation inside
`unsafe { }`. This was correct — the operation is unsafe, and the fixture should
exercise the detection. But it also meant the fixture suite provided zero coverage
of the case where a same-named call appears in safe-context code. Green CI on 600+
fixtures said nothing about that case, because the author had not encoded it.

The implication for fixture authorship: do not rely on positive-control fixtures
alone. Add **adversarial negative controls** that probe the disciplines the
detector might miss:

- a same-named call in safe context (no unsafe block, not inside unsafe fn)
- a same-named function definition, not a call
- the guard pattern applied to a different binding than the operation
- the operation keyword inside a doc comment or string literal
- a path segment that resembles but is not the target module

These are the cases a fresh-crate dogfood run will surface if the detector is
wrong. Encoding them upfront is cheaper than discovering them post-release.

---

## Surfacing is not suppression

A parallel lesson from the owner-settled stance decisions in this session: when a
correct card feels noisy, the answer is to adjust the surfacing layer (comment-plan
selection, PR-summary budget, ub-review group/rank pass), not to delete the card
from the evidence layer.

The evidence layer (`ReviewCard`) must be complete. Every card that reflects a
genuine obligation gap belongs there. The surfacing layer applies audience-specific
selection, ranking, and grouping. Suppressing evidence to reduce perceived noise
erodes the instrument; grouping and ranking reduce cognitive load without loss of
information.

This is encoded as the "Surfacing is not suppression" principle in SPEC-0028.

---

## The merge-train / controller-context tax

An operational lesson from the fixture-PR phase: the monolithic
`policy/calibration.toml` serializes fixture PRs. Two PRs that both add a new
calibration entry will conflict on that file, forcing rebase. On a large session
with many parallel agent-built fixture PRs, this serializes what would otherwise
be parallel merge work.

Mitigations:

- The controller serializes fixture PRs (merge one before queuing the next).
- Alternatively: per-fixture registration files (a design proposal for a future
  lane; see the filed issue on calibration-conflict tax).
- As a reminder: large fixture batches should be sequenced in the PR queue, not
  parallelized in the commit graph.

---

## Cross-references

- Detector discipline D1–D5 encoded in:
  `docs/specs/appendices/UNSAFE-REVIEW-SPEC-0005-appendix-operation-family-registry.md`
- Surfacing-vs-suppression principle in:
  `docs/specs/UNSAFE-REVIEW-SPEC-0028-delivery-surfaces-and-ease-of-use.md`
- Black-box/white-box pairing + clearly-correct/stance partition in:
  `docs/contributing/AGENT-ORCHESTRATION.md` (sections 13–14)
- Fixture-suite blindness note in: `CLAUDE.md` (Fixtures section)
- Capstone validation summary in: `docs/status/VALIDATION_CLOSEOUT.md`
- ADR for the syntax-first detection proposal:
  `docs/adr/UNSAFE-REVIEW-ADR-0009-syntax-first-detection.md`
