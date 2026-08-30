---
name: compile-work-spec
description: Use when issue design has converged to compile accepted decisions into the issue-linked work spec defined by SPEC-0044 and issue #1900 before admitting a writer.
---

# Compile Work Spec

Use when issue design has converged and a bounded delivery contract is needed before a writer is admitted. If the design is not actually settled, return to the issue instead of compiling.

## Triggers

- A converged issue has settled scope, invariants, and acceptance but no issue-linked work spec.
- The next step is to admit one writer for one PR.

Do not use this skill for mechanical fast-path work that needs no stable contract, or when unresolved discussion or speculative options still dominate.

## Workflow

1. Confirm the issue premise is current: re-read the controlling GitHub issue, its current disposition, exact base SHA, overlapping PRs, and linked specs/ADRs/plans.
2. Compile accepted decisions into the issue-linked work spec defined by `docs/specs/UNSAFE-REVIEW-SPEC-0044-issue-linked-work-specs.md` and its version-one shape `docs/schemas/issue-work-spec.schema.json`, as described in issue #1900.
3. Preserve: issue URL, basis SHA, source references, stable `INV-*` and `AC-*` IDs, included and excluded scope, proof commands, integration expectations, risk and mitigation, rollback strategy, and claim boundary.
4. Exclude unresolved discussion and speculative options from builder authority. Do not copy product specs into the work spec; link them as `linked_specs` or `linked_adrs`.
5. Validate offline shape with `cargo run --locked -p xtask -- check-work-specs`. No GitHub or network access is required for this check.
6. Do not create a second work contract, packet, or database. The work spec plus the GitHub issue is the bounded contract; `.allow` linkage is graph visibility only.

## Output shape

Use the canonical minimal example `plans/work-specs/examples/UNSAFE-REVIEW-WORK-1900.toml` and the schema fields `schema_version`, `issue`, `kind`, `objective`, `user_outcome`, `claim_boundary`, `scope`, `invariant`, `acceptance` (with `proof`), `integration`, `risk`, and `rollback`. Optional fields include dependencies, blockers, affected files or symbols, linked specs/ADRs, compatibility posture, and later `delivery` links.

## Examples

- A converged substantive issue without a work spec selects `compile-work-spec`, not writer admission directly.
- A typo fix (`docs/README.md`, one-line correction) uses the mechanical fast path and skips this skill; the short issue and direct PR are sufficient.
- When research helpers ran, their bounded results (`bounded-subagent-brief-v1` / `bounded-subagent-result-v1`) are synthesized into scope, proof, and risk, with contradictions preserved in the issue rather than hidden in the work spec.

## Boundaries

- GitHub issues remain the live research and portfolio surface; the work spec does not replace them.
- No label, current goal, role, or model is readiness authority. Admit the writer only when the issue plus the validated work spec are admitted together.
- Do not introduce branch or worktree creation, implementation, review, publication, merge, or cleanup behavior.
- Manual and non-Codex workflows remain valid: authoring `plans/work-specs/examples/<name>.toml` and running the checker by hand is sufficient.

## Claim boundary

This skill makes the contract-compilation step discoverable and reviewable. It does not prove that the compiled work spec is correct or that the implementation will satisfy it.

Reference: `docs/specs/UNSAFE-REVIEW-SPEC-0044-issue-linked-work-specs.md`, `docs/schemas/issue-work-spec.schema.json`, `plans/work-specs/examples/UNSAFE-REVIEW-WORK-1900.toml`, `AGENTS.md`, `docs/contributing/LIFECYCLE_SURFACE_MAP.md`.
