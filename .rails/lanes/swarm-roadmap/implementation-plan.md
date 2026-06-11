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
  - check-pr / source-divergence posture is installed and used as the routine
    swarm preflight.
  - PR body normalization, review-state hygiene, agent operating style, and
    single-contributor review-block handling are documented.
  - analyzer burst audit scaffolding is landed and has a follow-up status note.
- **H1: analyzer burst audit**
  - grouped family audit tables landed in the post-burst analyzer audit.
  - missing stale/wrong-target controls were recorded and later converted into
    the first evidence-applicability helper sequence.
- **H2: dogfood first-class lane**
  - post-burst snapshot, focused reruns, no-card control, and triage taxonomy
    are landed as dogfood report artifacts.
  - routine dogfood work should now convert one real observation into one
    fixture/control or route wording correction at a time.
- **H3: evidence applicability model**
  - shared subject identity / dominance / staleness model is documented.
  - initial helper-factoring sequence is implementation-backed for
    `unwrap_unchecked`, UTF-8 unchecked conversion, `get_unchecked`,
    `NonNull::new_unchecked`, `MaybeUninit::assume_init`, `Vec::set_len`, and
    `transmute` / `transmute_copy`.
- **H4+: disciplined family depth, output coherence, receipts, agent/editor, CI/process, maintainability, and promotion readiness**

## Seed backlog status

### P0 lock-in

1. landed: align `check-pr` gate usage with `--locked`
2. landed: add swarm PR body template
3. landed: document review-bot quota comments as no-review state
4. landed: add post-burst analyzer audit handoff
5. landed: add swarm digest template
6. landed: classify PR lanes for swarm work
7. landed: add analyzer PR checklist

### P1 audit + dogfood + evidence model

8. landed: group recent analyzer PRs by family
9. landed: record missing stale/wrong-target controls
10. active ongoing rule: fill one missing control per audited family only from
    concrete fixture or dogfood pressure
11. landed: add post-burst dogfood snapshot report
12. landed: add dogfood triage taxonomy and no-unsafe control target
13. landed: define evidence applicability model and initial helper-factoring
    sequence

### P2 projection and receipts hardening

14. landed: verify projection card-ID coherence across first-pr artifacts
15. landed: align saved-LSP hover and action payloads with ReviewCard-derived
    evidence and command-only projection boundaries
16. landed: add zero-card wording and no-overclaim artifact regressions
17. landed: harden receipt identity, staleness, duplicate, and command-hash
    audit behavior

### P3 maintainability

18. active: split operation detection from evidence applicability where the
    family already has fixture and dogfood pressure
19. active: move family-specific evidence logic into modules only as small,
    behavior-preserving slices
20. active: expand fixture naming, parser robustness, and perf rails when a
    concrete failure or dogfood observation requires it

## Current next work

The immediate roadmap is no longer "define the model" or "add the initial
helper sequence"; those rails are landed. Routine swarm work should now choose
the smallest useful slice from:

- a dogfood observation converted into one fixture/control,
- a family-specific applicability refactor that preserves current behavior,
- a verifier rail that keeps a ReviewCard projection honest,
- a CI/process budget edit that reduces duplicated cost,
- or a curated source promotion only after a coherent green swarm batch.

Do not broaden analyzer recognition because the roadmap has capacity. Broadening
needs a concrete fixture, dogfood observation, or verifier gap, and must retain
the advisory trust boundary.

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
