# UNSAFE-REVIEW-SPEC-0041: syntax-first dispatch architecture

Status: proposed
Owner: core / analysis
Created: 2026-06-15

## Purpose

This spec documents the analysis model for unsafe operation detection in
unsafe-review: AST-first dispatch, a bounded text fallback, and the discipline
checks applied at fallback entry. It does not introduce new behavior; it records
the architecture that ADR-0009 decided and that the detector-discipline
control-plane lane is making structurally enforced.

## Canonical source for D1–D5 and FM1–FM9

The discipline checks (D1–D5) and the failure-mode taxonomy (FM1–FM9) are defined
in the SPEC-0005 appendix:

> `docs/specs/appendices/UNSAFE-REVIEW-SPEC-0005-appendix-operation-family-registry.md`

That appendix is the **canonical** and normative source. This spec references it
and does not redefine, expand, or modify the obligation set. An earlier draft of
the control-plane work proposed a 9-obligation schema; that idea is **rejected**.
The D1–D5 set from SPEC-0005 appendix is the correct obligation set and must not
be replaced with a larger or divergent list.

## Analysis model

The analyzer processes a changed diff and the surrounding source in three
ordered phases:

### Phase 1: AST-first dispatch

For each candidate detection site, the analyzer attempts the `ra_ap_syntax`
(rust-analyzer AST) path first. The AST path has access to node kind, call
expression structure, unsafe-scope enclosure, comment trivia, and binding spans
without type resolution or macro expansion of the target repository.

When the AST path produces a result — whether a positive detection or a clean
miss — that result is authoritative. The text fallback path is not entered for
that site.

The AST path's structural properties naturally enforce D1 (unsafe-scope via
`SyntaxKind::UNSAFE_KW`), D2 (call-vs-definition via `SyntaxKind::CALL_EXPR` vs
`SyntaxKind::FN`), D4 (comment masking via trivia kinds), and D5 (path
segment anchoring via `PathExpr` segment traversal). D3 (same-receiver/origin)
requires domain-specific binding analysis but is considerably easier to apply
precisely when the AST structure is available.

### Phase 2: Bounded text fallback

The text fallback path is entered only when the AST path explicitly signals it
cannot parse the relevant syntax — for example, when the relevant node kind is
`Unknown`, indicating the site is inside a heavily macro-expanded form opaque to
`ra_ap_syntax`.

Fallback entry is a diagnostic aperture: the dispatcher logs when fallback is
used so coverage of fallback-dependent detections is observable.

### Phase 3: Discipline checks at fallback entry

At the moment the fallback path is entered for a candidate site, all applicable
D1–D5 discipline checks from the SPEC-0005 appendix are applied centrally before
any detector logic runs:

- **D1 (unsafe-scope gate):** The candidate line must be inside an `unsafe {}`
  block or an `unsafe fn` body in the compacted source context. A same-named
  safe-context call is not the stdlib operation and must not proceed.
- **D2 (definition-vs-call gate):** The text match must not be on a function
  definition header (`fn NAME(`). Only call-site occurrences proceed.
- **D3 (same-receiver/origin):** Evidence extracted by the text path is tagged
  with its binding origin before it can be used to discharge an obligation. A
  guard on a different pointer, receiver, index, slot, or destination does not
  discharge the candidate site.
- **D4 (string/comment masking):** The matched text must not appear exclusively
  inside a string literal or comment span. The fallback dispatcher verifies the
  match position is not inside a comment trivia region or string token.
- **D5 (word/segment-anchored path matching):** Path-based matches are anchored
  at the correct module segment position. A call path whose tail segment spells a
  module name (e.g., `registry_ptr::read_entry`) does not match `std::ptr`.

A text-path candidate that does not pass all applicable discipline checks is
discarded at the dispatcher level; it never reaches `ReviewCard` construction.
Individual detector implementations in the text path must not re-implement these
checks — they are guaranteed to have been applied before the detector runs.

### No-duplication invariant

A text-path result for a site already classified by the AST path is discarded.
This makes the existing precedence rule from the SPEC-0005 appendix
("Syntax-backed detections are authoritative when available") an architecture
invariant enforced at the dispatch level, not a post-hoc deduplication filter.

## Semantic enrichment: optional and future

Semantic enrichment — type resolution, trait resolution, MIR analysis, or macro
expansion of the target repository — is not part of the current analysis model.

A future `--semantic` flag or `--lsp-enrich` mode MAY add semantic enrichment as
an optional path. Any such path:

- MUST clearly label its evidence source in every output surface:
  `evidence_source: "syntax"` vs `evidence_source: "semantic"`.
- MUST NOT change the trust boundary: results derived from semantic enrichment
  remain advisory heuristics, not memory-safety proofs or UB-free claims.
- MUST NOT become a required default. Syntax-first / build-free analysis stays
  the default. No mandatory type-aware / MIR / `cargo build` path is introduced.
- MUST be documented as a separate surface-flag with its own xtask gate before
  promotion.

## Trust boundary

unsafe-review is an **advisory** static-review tool. This spec preserves that
boundary without exception:

- unsafe-review does not **prove** code safe, memory-safe, or free of undefined
  behavior.
- unsafe-review does not claim **UB-free** or **Miri-clean** status for any
  analyzed site.
- unsafe-review does not perform **site execution** or report witness execution
  results unless a separate witness receipt (from Miri, cargo-careful, Loom,
  Shuttle, or a named tool) is attached and imported via the receipt system.
- unsafe-review does not assert **calibrated precision or recall**. Fixture
  calibration is obligation-level evidence for specific detection shapes; it is
  not a global accuracy claim.
- The default analysis path remains syntax-first and build-free. No PR run of
  unsafe-review requires the target repository to build successfully.
- No unsafe-review output surface **blocks** merges or posts comments by default.
  Blocking and comment posting are explicit opt-in behaviors.

The ReviewCard is the single truth object. All output surfaces — JSON, SARIF,
markdown, LSP diagnostics, agent packets, comment plan, witness plan, badges,
baselines — project from the same card. No second truth surface is permitted.

## Implementation tracking

Implementation of this architecture and the associated detector-discipline ledger
and gate are tracked by the control-plane lane. The sequence is:

- SPEC-0041 (this spec): document the dispatch architecture. Status: proposed.
- SPEC-0040: detector-discipline ledger schema and scaffold. Status: planned.
- PR-5 (control-plane lane): xtask gates for the ledger. Status: planned.

See `.rails/lanes/control-plane/implementation-plan.md` for the full sequence.
