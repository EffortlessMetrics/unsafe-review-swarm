# Bun stable-byte cockpit implementation plan

## Scope

Keep the manual-candidate and stable-byte workflow useful for Bun fork
burndown lanes. This lane is about checked source-of-truth alignment and
copy-only cockpit artifacts, not broad analyzer expansion.

The cockpit path is:

```text
manual candidate or ReviewCard
-> stable-byte family
-> proof mode
-> fix boundary
-> PR aperture
-> repair or witness queue
-> stable-byte seed ledger state
-> next implementer action
```

## Current rails

1. Manual candidates stay `source = manual`, `manual_candidate = true`, and
   `analyzer_discovered = false`.
2. Stable-byte fields, proof mode, fix boundary, PR aperture, oracle map,
   test targets, and do-not-touch lists stay advisory handoff data.
3. `first-pr` renders the manual-candidate cockpit through
   `manual-candidates.json`, `manual-repair-queue.json`, `tokmd-packets.json`,
   `pr-summary.md`, `github-summary.md`, `witness-plan.md`, and
   `review-kit.json`.
4. `comment-plan.json`, ReviewCard repair queues, policy reports, SARIF, saved
   LSP, and ReviewCard JSON remain ReviewCard-only unless an explicit future
   spec promotes a different boundary.
5. Dogfood stable-byte seed rows track ledger state, owner lane, proof mode,
   suggested first PR, and triage labels without claiming Bun runtime proof.

## Next useful slices

- Keep committed Bun manual candidate examples and their verifier in sync.
- Tighten cockpit/verifier wording when a surface can overclaim manual
  candidates as analyzer findings, witness execution, proof, repair success,
  policy readiness, UB-free status, Miri-clean status, or site execution.
- Convert only dogfood-backed stable-byte seeds into one fixture/control or
  one narrow heuristic at a time.
- Keep `ripr` and `tokmd` requirements as interface rails until checked
  integrations exist.
- Prepare source promotion only as a curated green batch after the user-facing
  manual-candidate cockpit flow materially improves.

## Non-goals

- No broad stable-byte analyzer expansion without fixture or dogfood pressure.
- No default witness execution.
- No automatic comments.
- No source edits.
- No default blocking policy.
- No proof, UB-free, Miri-clean, site-execution, calibrated precision/recall,
  repair-success, or policy-readiness claims.
- No routine implementation PRs directly in `unsafe-review`.

## Proof commands

- `cargo test -p unsafe-review-core manual_candidate`
- `cargo test -p unsafe-review-core receipt`
- `cargo test -p unsafe-review-cli candidate`
- `cargo test -p unsafe-review first_pr`
- `cargo test -p xtask manual_candidate`
- `cargo run --locked -p xtask -- check-manual-candidate-examples`
- `cargo run --locked -p xtask -- check-dogfood`
- `cargo run --locked -p xtask -- check-pr`
- `cargo run --locked -p xtask -- source-divergence`
- `git diff --check`

## Claim boundary

This lane proves only that the Bun manual-candidate cockpit and stable-byte
handoff rails are linked, checked, and advisory. It does not prove a Bun site
executed, does not prove UB, does not prove memory safety, does not run Miri or
Bun, and does not make manual candidates policy inputs.
