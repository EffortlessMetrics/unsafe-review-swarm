# Claude Code adapter

This file only maps Claude Code onto the repository protocol. It owns no
durable project state and does not redefine repository authority.

## Start here

Read [`AGENTS.md`](AGENTS.md) first; it wins on conflict. Then use:

- [`docs/contributing/LIFECYCLE_SURFACE_MAP.md`](docs/contributing/LIFECYCLE_SURFACE_MAP.md)
  to find the authority and durable artifact for the current transition;
- [`docs/contributing/AGENT-ORCHESTRATION.md`](docs/contributing/AGENT-ORCHESTRATION.md)
  for detailed orchestration and verification examples;
- the selected GitHub issue or PR plus its linked spec, plan, ADR, or work spec
  for the current bounded contract.

GitHub owns the live portfolio. `.allow` supplies durable graph evidence, not
scheduling; `.rails` is a read-only parity archive. Do not copy live issue,
branch, commit, release, or session state into this adapter.

## Choose the role

- For inspection, planning, triage, review, or multi-item coordination, act as
  the **coordinator**: reconstruct live state, select and sequence bounded work,
  assign one writer per mutation surface, join exact-head evidence, and retain
  publication, merge, reconciliation, and cleanup judgment.
- Act as a **bounded worker** only when given one issue or accepted work
  contract, exact base and worktree, objective, and explicit read-only or
  mutation scope. Return files, exact base/head, proof, uncertainty, claim
  boundary, and cleanup state; do not widen the assignment.

Claude-specific helpers under `.claude/agents/` are optional runtime adapters,
not repository roles or proof. Use manual or single-agent execution when it is
clearer. Model choice and tool availability are runtime details.

## Fixtures, calibration, and dogfood

The fixture-suite-blindness doctrine lives in
[`ANALYZER-LEARNINGS.md`](docs/contributing/ANALYZER-LEARNINGS.md#fixture-suite-blindness),
with the detector negative-control contract in the
[SPEC-0005 operation-family registry](docs/specs/appendices/UNSAFE-REVIEW-SPEC-0005-appendix-operation-family-registry.md#negative-control-requirement).
This runtime adapter does not restate either authority.

## Required boundaries

`unsafe-review-swarm` develops; `unsafe-review` publishes. Before routine swarm
implementation, run `cargo run --locked -p xtask -- source-divergence`, inspect
branch/worktree/PR ownership, and use an isolated worktree when the primary
checkout is not the admitted mutation surface.

The comprehensive local sequence is formatting, workspace Clippy, workspace
tests, then `cargo run --locked -p xtask -- check-pr`; focused checks should run
first. Review and proof apply only to the exact head and become stale after a
relevant mutation. Hosted checks and live GitHub policy, not an agent verdict,
own integration.

`ReviewCard` remains the single product truth. The tool is advisory: no default
witness execution, comments, source edits, or blocking policy, and no safety,
UB-free, Miri-clean, site-execution, proof, or calibrated-accuracy claim without
the specifically required evidence. Release, publication, tagging, deployment,
and source promotion require separate authority.
