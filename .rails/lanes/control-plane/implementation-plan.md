# Detector-discipline control-plane lane — implementation plan

> **Status (2026-06-15): closed.** PR-0–PR-6 plus the follow-ups PR-A/PR-B/PR-C
> shipped the control plane (three discipline ledgers + three enforcing gates).
> PR-7 (calibration de-bottleneck) is deferred to its own phased lane, tracked in
> issue #1712. See [`docs/status/CONTROL_PLANE_CLOSEOUT.md`](../../../docs/status/CONTROL_PLANE_CLOSEOUT.md).

## Lane thesis

Establish a single-truth control plane that makes detector discipline, product
stances, and surface projections **structurally enforced** instead of fixed by
hand. The substring-anchoring bug class (a detector firing on a token without
checking unsafe scope, call-vs-definition, receiver/origin, string/comment
masking, or word boundary) is asymptotic by design under the syntax-first,
build-free analyzer: each anchoring fix tends to reveal the next anchoring gap.
The right response is an invariant, not a 51st point fix — a gate that forces
every detector to declare its discipline obligations and negative controls, and
forces every settled stance to keep its proof.

Everything in this lane is advisory-boundary-safe: no memory-safety proof, no
UB-free / Miri-clean / site-execution / calibrated-precision claim is added to
any surface. The gates validate **process discipline and ledger shape**, never
soundness.

## Reuse (do not reinvent)

- **D1–D5 discipline checks** and the **FM1–FM9 failure-mode taxonomy** already
  exist in
  `docs/specs/appendices/UNSAFE-REVIEW-SPEC-0005-appendix-operation-family-registry.md`.
  This lane references that appendix as canonical; it does not define a divergent
  obligation set.
- **ADR-0009** (`docs/adr/UNSAFE-REVIEW-ADR-0009-syntax-first-detection.md`)
  already records the syntax-first decision at status `proposed`; PR-1 promotes
  it to `active`.

## Refuter pre-corrections (verified — do not re-introduce)

- **No SPEC-0036.** It was dropped 2026-06-12 as redundant with SPEC-0005/0028.
  This lane uses the free numbers **SPEC-0040 / 0041 / 0042**. Do not recreate
  SPEC-0036 or invent a 9-obligation schema; reuse the D1–D5 set.
- **36 operation families** in `domain/operation.rs` (35 named + `Unknown`), not
  37/49.
- **`StaticMut` is NOT a duplicate bug.** `StaticMut` appears once in
  `UnsafeSiteKind` (operation.rs enum starting ~line 4) and once in
  `OperationFamily` (enum starting ~line 35) — two distinct enums legitimately
  sharing a variant name. Do not "fix" it.

## Sequence (one PR each; gates born informational, enforcement flipped on later)

Ordering principle: ADR/specs → ledgers → gates → high-risk migration →
calibration de-bottleneck. Each new gate is introduced **passing on an
empty-or-scaffold ledger**; enforcement is turned on only once the data it
guards exists, so every PR stays green and merge-safe.

- **PR-0 (this PR) — lane anchor.** Register the lane in `.rails/index.toml`,
  add the `[[work_item]]` in `.rails/goals/active.toml`, and land this plan. No
  code.
- **PR-1 — ADR-0009 → active + SPEC-0041.** Promote ADR-0009 to `active`; land
  SPEC-0041 (syntax-first / semantic-light dispatch architecture) citing the
  SPEC-0005 appendix as the canonical D1–D5 source. Docs only — no analyzer
  code, no new gate in this PR. Register SPEC-0041 as an artifact in
  `.rails/index.toml` + `policy/doc-artifacts.toml`.
- **PR-2 — SPEC-0040 + `policy/detector-contracts.toml` scaffold (empty).**
  Define the detector-contracts ledger schema (operation_family, obligations
  drawn from D1–D5, positive_fixtures, negative_fixtures, surfaces, evidence,
  review_after, plus an exceptions shape) and create the empty schema-valid
  scaffold. No families registered yet.
- **PR-3 — `policy/stance-decisions.toml`.** Encode the settled stances
  (debug_assert != runtime guard, SAFETY-comment != executable guard,
  static-mention != test reach, receipt != proof, owner-cards grouped-not-hidden,
  severity-from-card-class, formal-tool site_reached requires provenance), each
  with rationale / owner / linked-tests / linked-spec / review-after /
  exception-path.
- **PR-4 — `policy/spec-coverage.toml`.** Single-truth projection map: field →
  canonical pipeline source → projecting surfaces, as a checked coverage map.
- **PR-5 — xtask gates.** `check-detector-contracts`, `check-stance-decisions`,
  `check-spec-coverage` (and `check-spec-exceptions` if warranted), wired into
  the `check-pr` bundle. Born informational on the empty/scaffold ledgers.
- **PR-6 — high-risk family migration.** Register the high-risk detector
  families first (get_unchecked, ptr::copy / copy_nonoverlapping, mem::zeroed /
  transmute, Vec::set_len, unsafe-fn owner cards, FFI, stable-byte) into the
  contract ledger with their negative controls; flip enforcement on for the
  registered set.
- **PR-7 — per-fixture `meta.toml` + generated calibration aggregate.** Remove
  the `policy/calibration.toml` serialization bottleneck so parallel fixture PRs
  stop colliding on the shared registry file.

## Proof commands (per PR; this anchor PR runs the goals/pr subset)

```
cargo run --locked -p xtask -- check-goals
cargo run --locked -p xtask -- check-pr
cargo run --locked -p xtask -- source-divergence
git diff --check
```

## Boundaries

- Advisory only. The gates check ledger shape and discipline declarations, never
  memory-safety / UB / soundness. No surface gains a proof / UB-free / Miri-clean
  / site-execution / calibrated-precision claim.
- Syntax-first stays the default analysis path. Optional semantic enrichment is
  case-by-case; no mandatory type-aware / MIR / `cargo build` path is introduced.
- Stances are owner-decided. The stance-decisions ledger records them so a future
  agent cannot silently weaken a deliberate stance as a "noise fix."
