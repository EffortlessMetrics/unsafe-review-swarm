# Swarm roadmap lane: long-running workbench program

## Purpose

`unsafe-review-swarm` is the long-running internal workbench for analyzer correctness, ReviewCard alignment, fixture/dogfood validation, agent/editor projections, CI rails, and source-of-truth hardening.

`unsafe-review` remains the curated public source/release repository.

This lane tracks swarm-internal execution and alignment work. It is **not** a release/version plan and does not imply publication commitments.

## Swarm operating rule

When there is no narrower owner instruction, execute work in this order:

1. Keep `main` green and `source-divergence` understood.
2. Lock in recent work with tests, fixtures, ledgers, and docs.
3. Audit recent analyzer behavior before broadening it.
4. Prefer dogfood-backed analyzer work over speculative expansion.
5. Keep ReviewCard as source-of-truth for every projection.
6. Preserve the advisory trust boundary.
7. Do not publish, version, or promote support claims from swarm-only work.

## Horizon map

- **H0: lock-in and post-burst alignment**
  - check-pr / source-divergence posture
  - PR body normalization and review-state hygiene
  - analyzer burst audit scaffolding
- **H1: analyzer burst audit**
  - grouped family audit tables
  - missing stale/wrong-target controls recorded
- **H2: dogfood first-class lane**
  - routine snapshot reports
  - triage taxonomy and regression summaries
- **H3: evidence applicability model**
  - shared subject identity / dominance / staleness model
  - helper-factoring and target-identity declaration norms
- **H4+: disciplined family depth, output coherence, receipts, agent/editor, CI/process, maintainability, and promotion readiness**

## Seed backlog

### P0 lock-in

1. align `check-pr` gate usage with `--locked`
2. add swarm PR body template
3. document review-bot quota comments as no-review state
4. add post-burst analyzer audit handoff
5. add swarm digest template
6. classify PR lanes for swarm work
7. add analyzer PR checklist

### P1 audit + dogfood + evidence model

8. group recent analyzer PRs by family
9. record missing stale/wrong-target controls
10. fill one missing control per audited family (small scoped PRs)
11. add post-burst dogfood snapshot report
12. add dogfood triage taxonomy and no-unsafe control target
13. define evidence applicability model and helper-factoring follow-ups

### P2 projection and receipts hardening

14. verify projection card-ID coherence across first-pr artifacts
15. align LSP hover phrasing with explain output
16. add zero-card wording regression
17. harden receipt identity / staleness / duplicate handling

### P3 maintainability

18. split operation detection from evidence applicability
19. move family-specific evidence logic into modules
20. expand fixture naming, parser robustness, and perf rails

## Analyzer PR checklist (required for analyzer behavior PRs)

- [ ] operation family named
- [ ] obligation named
- [ ] newly accepted evidence described
- [ ] evidence target identity described
- [ ] positive fixture added
- [ ] negative fixture added
- [ ] stale evidence control added
- [ ] wrong-target control added where applicable
- [ ] comment-only behavior covered where applicable
- [ ] fixture golden updated
- [ ] calibration ledger updated
- [ ] support-tier row changed only within current claim level
- [ ] public wording reviewed for overclaim
- [ ] dogfood observation linked or marked not-yet-dogfooded
- [ ] no witness execution
- [ ] no automatic comments
- [ ] no source edits
- [ ] no default blocking
- [ ] no safety/UB-free/Miri-clean/site-execution/calibrated-precision claim

## Proof commands

- `cargo run --locked -p xtask -- source-divergence`
- `cargo run --locked -p xtask -- check-pr`
- `cargo run --locked -p xtask -- check-goals`
- `cargo run --locked -p xtask -- check-doc-artifacts`
- `git diff --check`
