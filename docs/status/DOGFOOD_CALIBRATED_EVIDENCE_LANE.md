# Dogfood-calibrated evidence lane

Date: 2026-05-18

Status: closed experimental lane; broader calibration continues

## Lane Goal

Make `unsafe-review` a repeatable unsafe-review evidence loop for Rust PRs:

```text
changed unsafe-adjacent Rust
-> ReviewCard
-> reviewer action
-> optional witness receipt
-> repo outcome delta
-> calibrated policy posture
```

This lane does not turn `unsafe-review` into a UB prover, a Miri replacement, a
general security scanner, or an unsafe counter. It makes unsafe Rust review
evidence structured, comparable, and honest enough to support later policy.

The core sentence remains:

```text
unsafe-review finds unsafe Rust changes missing a safety contract, guard, test,
or witness.
```

## Current Baseline

These surfaces have landed and are no longer the active lane:

- review-card correctness and fixture goldens
- advisory PR artifacts: cards JSON, PR summary, SARIF, and comment-plan
- saved LSP projection
- bounded agent packet projection
- receipt schema/import foundation
- repo posture snapshots, badge JSON, and exact policy matching
- first saved-snapshot outcome comparison
- fixture calibration manifest
- first real-crate dogfood slice

The authoritative proof and limits remain in:

- `docs/status/SUPPORT_SUMMARY.md`
- `docs/status/SUPPORT_TIERS.md`
- `docs/status/OBJECTIVE_AUDIT.md`
- `docs/handoffs/2026-05-18-dogfood-calibrated-evidence-v0.6.md`
- `docs/handoffs/2026-05-18-real-crate-dogfood-v0.6.md`
- `docs/handoffs/2026-05-18-repo-policy-v0.4.md`
- `docs/handoffs/2026-05-18-witness-receipt-import-v0.5.md`
- `docs/handoffs/2026-05-18-fixture-calibration-v0.6.md`

## Product Loop

The lane is complete when this loop is repeatable:

1. Scan a PR diff or repo snapshot.
2. Emit `ReviewCard`s as the single source of truth.
3. Project cards into existing PR, LSP, agent, repo, and policy artifacts.
4. Verify generated artifacts and trust boundaries.
5. Import scoped witness receipts without executing witness tools by default.
6. Compare saved snapshots and explain outcome movement.
7. Record dogfood evidence and known limits.
8. Update support tiers only when fixture, dogfood, receipt, or outcome proof exists.
9. Keep blocking policy off until calibration justifies it.

## Done Criteria

This lane is done when:

- dogfood has a manifest-backed corpus of selected real crates and PR diffs
- dogfood artifacts are mechanically validated
- saved-snapshot outcome JSON and Markdown are pinned and explain movement
- receipt matching reports matched, unmatched, expired, stale, wrong-identity,
  wrong-tool, weaker-than-required, command-hash-mismatch, duplicate, and invalid
  receipt metadata
- outcome comparison can report receipt-strength movement without overclaiming
- repo inventory JSON and Markdown are pinned for later posture reporting
- advisory no-new-debt can emit a non-blocking policy report
- support tiers distinguish fixture-backed, dogfood-backed, and calibrated surfaces
- no output claims soundness, UB-free status, Miri-clean status, target-feature
  availability, site execution, or policy readiness without exact evidence

## Closed Sequence

This lane proceeded PR by PR:

1. Define this lane in docs/source-of-truth.
2. Close or park stale candidate PRs as option inventory.
3. Add a real-crate dogfood corpus manifest.
4. Add `xtask` validation for dogfood receipts and snapshots.
5. Pin outcome comparison JSON and Markdown.
6. Add outcome reason text explaining why cards improved or regressed.
7. Add receipt audit: matched, unmatched, expired, stale, wrong identity, wrong
   tool, weaker-than-required, command-hash-mismatch, duplicate, and invalid
   receipt metadata.
8. Add receipt-strength movement to outcome comparison.
9. Pin repo inventory JSON and Markdown.
10. Add an advisory no-new-debt policy report that remains non-blocking by default.
11. Promote only support-tier surfaces with direct proof.
12. Close the lane with a calibration handoff.

## Out Of Scope

Do not add these in this lane:

- default blocking CI
- automatic inline comments
- automatic source edits
- witness execution by default
- broad suppressions or broad baseline matching
- publish, tag, or release steps
- rustc_private or mandatory MIR
- full repo-wide live LSP diagnostics
- calibrated precision or recall claims without measured evidence

## Current Evidence Posture

Current evidence is experimental and dogfood-backed in places. It is not
calibrated. The repo may say that `unsafe-review` can emit and compare advisory
review evidence over a small selected corpus. It must not say that the corpus
proves broad precision, broad recall, safety, soundness, Miri success, witness
success, or policy readiness.

Support-tier promotion must wait until the corresponding proof is present in
fixtures, dogfood receipts, outcome reports, or receipt validation.
