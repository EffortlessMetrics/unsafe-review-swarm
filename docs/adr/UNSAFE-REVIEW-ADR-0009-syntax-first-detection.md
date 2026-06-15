# UNSAFE-REVIEW-ADR-0009: Syntax-first detection — make the AST path primary, the text fallback bounded and explicit

Status: active
Date: 2026-06-15
Owner: core/architecture
Linked specs:
- ../specs/UNSAFE-REVIEW-SPEC-0005-hazard-taxonomy-and-obligations.md
- ../specs/appendices/UNSAFE-REVIEW-SPEC-0005-appendix-operation-family-registry.md
Linked docs:
- ../contributing/ANALYZER-LEARNINGS.md
Linked PRs:
- context: #1672–#1707 (card-correctness session that motivated this proposal)

## Decision

unsafe-review stays **syntax-first and build-free by default.** It analyzes
syntax, tokens, unsafe-syntax patterns, diff hunks, and comments of the *target*
repository without performing target-repo type resolution, trait resolution, MIR
analysis, or macro expansion at scan time. This is a deliberate design choice, not
a limitation to be papered over.

What this means precisely:

- unsafe-review's own implementation is strongly-typed Rust (`ReviewCard`,
  `HazardKind`, `ObligationKind`, etc.) — it is not "type-free."
- The scanned repo's types exist in the Rust source but are not resolved by the
  analyzer at scan time. The tool does not require a successful `cargo build` of
  the target repo.
- The analyzer's view of the target is: structured syntax via `ra_ap_syntax` (AST
  nodes — call-expr, unsafe fn body, comment trivia, binding spans) for the
  primary path; substring matching on compacted whitespace for the fallback path.
  Neither path performs `rustc` type-check, MIR traversal, or trait resolution.

This posture is called **syntax-first** (AST is primary), **semantic-light** (no
full semantic model of the target), and **build-free** (no mandatory compilation
of the target). Substring/text matching is a **bounded, explicit, tested last
resort** — not the preferred detection path.

This ADR governs the AST-first dispatch change. It does NOT make full target-repo
type resolution mandatory for any path (PR, IDE, or agent). Optional semantic
enrichment MAY be added later (e.g., a `--semantic` flag or `--lsp-enrich` mode)
but MUST clearly label its evidence source (syntax-derived vs. semantic-derived) in
every output surface that claims enrichment, to preserve the trust boundary and
allow callers to understand what kind of evidence backs each card.

---

## Problem

The current detector architecture has two code paths for recognizing unsafe
operations:

1. **Syntax path** — backed by `ra_ap_syntax` AST traversal. When a node is
   recognized as a specific call expression, the AST provides scope,
   call-vs-definition, binding identity, and span type for free. False positives
   in this path are rare: the tree already encodes the structural properties that
   the discipline checks (D1–D5, SPEC-0005 appendix) require.

2. **Text fallback path** — per-line substring matching on `compact_whitespace`
   strings. This path operates on raw text and must explicitly re-derive every
   structural property the AST gives the syntax path for free. In practice,
   individual detectors in the text path have historically omitted one or more of
   the five discipline checks, leading to false positives on safe-context calls,
   function definition headers, different-binding guards, comment text, and
   path-segment mismatches.

The card-correctness session (#1672–#1707) found that almost all false positives
traced to the text fallback path missing at least one of D1–D5. The AST path had
none of the same failures. Yet the current architecture treats both paths as
co-equal and applies them independently per detector, allowing the text path to
emit a card even when the AST path would have rejected it.

## Proposal

Make the `ra_ap_syntax` AST path the **primary and authoritative** detection path.
The text fallback path becomes a **bounded, explicitly-scoped last resort** used
only when the AST path is unable to parse the relevant syntax (e.g. heavily
macro-expanded forms where `ra_ap_syntax` returns `Unknown`).

Concretely:

1. **AST-first dispatch.** For each detection site, attempt the AST path first.
   If the AST path produces a result (hit or clean miss), use that result and skip
   the text path entirely.

2. **Text fallback gated on AST failure.** Enter the text fallback only when the
   AST path explicitly signals it cannot parse the site (e.g. node kind is
   `Unknown`, the macro expansion is opaque). Log the fallback entry as a
   diagnostic aperture, not a normal path.

3. **Centralized discipline checks at dispatch.** The five discipline properties
   (D1: unsafe scope, D2: definition-vs-call, D3: same-origin, D4: string/comment
   masking, D5: path-segment anchoring) are enforced once at the text-fallback
   dispatch point, not re-implemented per detector. A text-path result that has
   not passed all applicable discipline checks is discarded before it reaches
   `ReviewCard` construction.

4. **No duplication.** A text-path result for a site already classified by the AST
   path is discarded. The existing "no duplicate" precedence rule (SPEC-0005
   Precedence section) becomes an architecture invariant, not a post-hoc
   deduplication filter.

## Evidence

- The five discipline failures identified in the card-correctness session
  (#1672–#1707) all occurred in the text fallback path.
- The `ra_ap_syntax` path, which already encodes scope, call-vs-definition, and
  binding identity, produced no equivalent false positives on the same codebase.
- The `ptr_read_path_segment_not_raw`, `*_safe_method_*_no_cards`, and
  `inline_unsafe_*_no_duplicate` fixture families all pin cases where the text
  path required explicit checks that the AST path handles structurally.

## Risks and mitigations

- **Coverage gap on macro-expanded forms.** The AST path may return `Unknown` on
  heavily macro-expanded call sites where the text path would catch the pattern.
  Mitigation: the text fallback remains available as a bounded last resort,
  explicitly gated on AST failure. The fallback's discipline checks prevent the
  false positive class; the coverage reduction is bounded to opaque macro forms.

- **Migration scope.** Moving all detectors to AST-first dispatch requires
  touching every operation-family module. This is a refactor, not a behavior
  change, but it carries test-regression risk. Mitigation: the existing ~600
  fixture calibrations serve as a regression suite; a per-family migration with
  per-family green gate between each family keeps the scope manageable.

- **`ra_ap_syntax` version coupling.** The AST path depends on `ra_ap_syntax`,
  which follows rust-analyzer releases. The crate is already a dependency (ADR-
  0001); this proposal does not introduce new coupling. It does make the coupling
  load-bearing for more detection paths, increasing the importance of the
  rust-analyzer version pin.

## Non-goals

- This ADR does not propose removing the text fallback path entirely. Macro-
  expanded forms and other AST-opaque sites have genuine value from text
  detection; the proposal bounds that path, not eliminates it.
- This ADR does not change the `ReviewCard` schema, detection families, or
  calibration table.
- This ADR does not propose new detection families or new obligations.
- This ADR does not change the trust boundary: the analyzer remains advisory
  source-text heuristic; the change improves precision without upgrading the
  claim level.

## Alternatives considered

### Per-detector discipline enforcement (current approach)

Each detector in the text path checks whichever discipline properties its author
remembered to check. This is fragile: adding a new detector requires the author to
know about and implement all five discipline checks. The card-correctness session
demonstrated that authors reliably miss one or more, and the failure is silent
(no gate catches a missing discipline check at authoring time).

**Rejected** as the status quo that produced the identified false-positive class.

### Discipline gate as a separate compile-time lint

Define a trait or type-system mechanism that forces each text-path detector to
declare which disciplines it enforces, and fail to compile if a discipline is
omitted without an explicit exemption reason. This would be strong but requires
significant trait/type scaffolding and shifts the burden to the type system rather
than centralizing the logic.

**Deferred.** Architecturally appealing but more complex than the dispatch-point
centralization. Could be a follow-up after the AST-first dispatch change proves
stable.

### Gate-based discipline enforcement (filed issue)

A separate proposal (see the "detector-discipline as a checked contract" issue)
proposes a gate that fails if a detector lacks negative controls for its applicable
disciplines. This is complementary to this ADR, not an alternative: the gate
prevents the class from regrowing via missing fixtures; this ADR prevents the
class from existing in the first place via the architecture.

---

Implementation of the detector-discipline spec (SPEC-0040), the dispatch architecture
spec (SPEC-0041), and the associated ledger and gate are tracked by the control-plane
lane (`detector-discipline-control-plane` in `.rails/index.toml`).
