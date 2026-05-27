# UNSAFE-REVIEW-SPEC-0019 — First-run cockpit (0.2.0 lane)

- Status: Accepted
- Last updated: 2026-05-21
- Owners: unsafe-review maintainers
- Depends on: 0001, 0002, 0008, 0009, 0010, 0011, 0012, 0013, 0016, 0024

## 1. Purpose

Define the 0.2.0 product lane as **first-run usability** for advisory unsafe PR review.

This spec standardizes the user-visible flow, required artifacts, output posture, and release-proof checks for a maintainer’s first successful run.

## 2. Non-goals

0.2.0 does **not** claim or require:

- UB proof.
- Memory-safety proof.
- Miri replacement.
- Policy-gate authority by default.
- Automatic source edits.
- Automatic comment posting.
- Default witness execution.
- Live LSP server or VS Code extension.
- Precision/recall calibration claims.

## 3. First-run command path

A successful first run MUST support this path:

```bash
cargo install unsafe-review --locked
unsafe-review doctor
unsafe-review first-pr --base origin/main
open target/unsafe-review/pr-summary.md
unsafe-review explain <card-id>
```

Implementations MAY use equivalent install/open commands by platform, but behavior and posture MUST remain equivalent.

## 4. First-pr advisory bundle contract

`unsafe-review first-pr` MUST emit an advisory bundle at `target/unsafe-review/` (or caller-provided output dir) containing:

- `cards.json`
- `pr-summary.md`
- `cards.sarif`
- `comment-plan.json`
- `witness-plan.md`
- `lsp.json` (saved projection; optional when no cards or no projection content)

Bundle shape MUST pass
`cargo run --locked -p xtask -- check-first-pr-artifacts <dir>`.

When cards are present, `pr-summary.md` and `github-summary.md` MUST include
top-card handoff commands for human explanation and bounded agent context:
`unsafe-review explain <card-id>` and
`unsafe-review context <card-id> --json`.

## 5. First-pr terminal summary contract

On successful bundle write, terminal output MUST include:

- Artifact directory.
- Card count.
- Top-card handoff commands (`unsafe-review explain <card-id>` and
  `unsafe-review context <card-id> --json` when present).
- `pr-summary.md` location.
- Trust boundary statement.

Minimum trust boundary wording intent:

- “static unsafe contract review only”
- “not memory-safety proof”

## 6. No-card honesty contract

When no changed gaps are found, all relevant user-facing surfaces MUST use aligned advisory wording and MUST avoid overclaims.

Required wording shape:

- “No changed unsafe-review gaps were found.”
- “This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.”

Forbidden wording examples:

- “All clear.”
- Any equivalent absolute-safety claim.

At minimum this applies to: terminal human output, `pr-summary.md`, `comment-plan` framing, repo posture/policy report wording.

## 7. Explain reviewer-note contract

`unsafe-review explain <card-id>` human output MUST be reviewer-first and structured as:

1. Why this card exists
2. Required safety conditions
3. Evidence found
4. Evidence missing
5. What would resolve this
6. What would not resolve this
7. Witness route
8. Trust boundary

The content MUST be obligation-specific and action-oriented, and MUST not claim closure without evidence.

JSON and human explain views MUST remain semantically aligned.

## 8. Doctor first-run readiness contract

`unsafe-review doctor` MUST function as readiness guidance, not a hard gate for optional witness tools.

Doctor output MUST include:

- Workspace/root readiness.
- Git/base-ref readiness.
- Cargo metadata readiness.
- Artifact directory writability.
- Witness tool availability hints (Miri, cargo-careful, sanitizer/concurrency/model-checking hints).
- Advisory policy mode posture.
- Trust boundary.

Missing optional witness tools MUST be informational (non-fatal) with explicit “routing still works; execution is not default” framing.

## 9. Support posture command contract

`unsafe-review support` MUST report support and limits in plain language, including:

- ReviewCard support posture.
- First-pr bundle posture (advisory).
- Receipt audit scope.
- Outcome comparison scope.
- Policy report advisory posture.
- Blocking/comment/witness defaults (all non-default).
- Live LSP deferred status.

## 10. Witness-plan reviewer readability contract

`witness-plan.md` MUST group cards by witness route and, per card, present:

- Card identity.
- Why this route.
- Suggested command.
- What route can show.
- What route cannot prove.
- Receipt hint.

Route groups SHOULD include: Miri/cargo-careful, sanitizers, Loom/Shuttle, Kani/Crux, human deep review, unsupported/manual.

## 11. Advisory posture invariants (0.2.0)

The following MUST remain true for the 0.2.0 lane:

- No default blocking CI behavior.
- No automatic comments.
- No automatic source edits.
- No default witness execution.
- No broad suppressions as first-run UX substitute.
- No safety/UB-free/Miri-clean/site-executed/proof claims.

## 12. Projection contract summary

Every first-run surface is a projection from ReviewCard.

- PR gate: hard-fail tool, artifact, schema, and trust-boundary failures; keep
  findings advisory by default.
- PR comments: write `comment-plan.json` first; cap at three candidates; changed
  lines only; no posting by default.
- Saved LSP: `lsp.json` is the 0.2.0 editor surface; diagnostics carry
  ReviewCard-derived evidence; hovers mirror reviewer-first explain; actions are
  command-only and contain no `WorkspaceEdit`.
- Agent packets: card-scoped, readiness-classified, bounded context, allowed
  repairs, do-not-do list, verify commands, and stop conditions.
- Badges: repo posture only; `unsafe-review` means open gaps and
  `unsafe-review+` means open gaps plus contract/guard/witness evidence-quality
  findings.

These surfaces MUST NOT create alternate analyzer truth.

## 13. Release-proof minimum for 0.2.0

Release readiness evidence MUST include smoke proof for:

- Install.
- `doctor`.
- `first-pr` bundle generation.
- `explain` on a produced card.
- `support`.
- First-pr artifact verifier.

It MUST also preserve advisory boundary language across outputs.

## 14. Acceptance checklist

0.2.0 first-run cockpit is acceptable when:

- Users can run the first-run path without architecture knowledge.
- Produced bundle is valid and navigable.
- Explain yields concrete next actions per card.
- Witness routes are actionable and bounded.
- Posture/limits are explicit and consistent.
- No surface overclaims safety/proof.

## 15. Out of scope for this spec

Detailed analyzer evolution, live LSP hardening implementation, editor extension packaging, and calibrated policy gating are deferred to post-0.2.0 lanes.
