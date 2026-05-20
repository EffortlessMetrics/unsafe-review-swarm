# 2026-05-20 - source-to-swarm first-run UX backfill

Scope: reconcile recent source-only first-run usability work from
`EffortlessMetrics/unsafe-review` back into `EffortlessMetrics/unsafe-review-swarm`
so swarm is again the default implementation and dogfood repo.

## Operating correction

This backfill treats the recent source-side first-run UX work as process drift,
not as bad product work.

From this point:

- normal implementation, UX, analyzer, doctor, explain, support, witness-plan,
  policy, receipt, LSP, dogfood, and coverage work starts in
  `unsafe-review-swarm`;
- `unsafe-review` receives curated promotions from green swarm PRs, release prep,
  publication receipts, public package/docs.rs/crates.io metadata, or urgent
  published-user hotfixes;
- source promotions must name their swarm PR or commit origin.

The preserved product boundary remains:

- advisory only,
- no witness execution by default,
- no automatic comments,
- no source edits,
- no default blocking policy,
- no safety, UB-free, Miri-clean, site-execution, or calibrated-precision claim.

## Reconciliation ledger

| Source PR | Source behavior | Swarm status | Reason / follow-up |
|---|---|---|---|
| #477 `docs(release): define 0.2.0 public usability target` | Defines 0.2.0 as first-run public usability rather than analyzer breadth, live LSP, or policy authority. | Superseded / deferred | Not cherry-picked into this backfill. Swarm PR #110 is the swarm-native first-run cockpit lane/spec path, so this branch avoids adding a duplicate source release-plan document. |
| #478 `xtask: verify first-pr artifact bundle` | Adds `xtask check-first-pr-artifacts`, validates the advisory bundle, caps comment-plan output, and checks overclaim wording. | Ported | Cherry-picked and conflict-resolved against swarm's stricter advisory artifact checks. |
| #479 `cli: include saved lsp in first-pr bundle` | Adds saved `lsp.json` to the `first-pr` / `review` bundle. | Ported | Preserves saved projection only; does not promote live LSP as a release surface. |
| #480 `cli: polish first-pr terminal summary` | Prints artifact directory, card count, top card, summary path, explain command, and trust boundary. | Ported | No analyzer behavior change. |
| #481 `output: standardize no-card advisory wording` | Standardizes no-card wording across user-facing outputs. | Ported | Keeps "no changed gaps" explicitly distinct from safety, UB-free, Miri-clean, or site-execution proof. |
| #482 `output: make explain reviewer-first` | Makes `explain` use reviewer-first sections for why the card exists, required conditions, evidence, resolution, non-resolution, route, and trust boundary. | Ported | Merged with swarm's current markdown renderer. |
| #483 `docs: add explain examples for common cards` | Adds examples for common card families and good/bad review outcomes. | Ported | Documentation only. |
| #484 `cli: add support posture command` | Adds `unsafe-review support` to expose advisory support tiers and non-claims. | Ported | Keeps support posture as a self-defense command, not policy authority. |
| #485 `output: group witness plan routes` | Groups witness-plan output by route and includes route limits and receipt hints. | Ported | Preserves witness planning only; no witness execution. |
| #486 `policy: explain report classifications` | Adds policy-report classification explanations, limitations, policy reasons, operation expression, next action, unmatched-baseline alias, and invalid-ledger fields. | Ported | Conflict-resolved to keep swarm's richer ledger/location/hazard/missing/route context and source's explicit classification/schema fields. |

## Validation target

This backfill should be validated with:

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p unsafe-review -- first-pr --root fixtures/raw_pointer_alignment --diff fixtures/raw_pointer_alignment/change.diff --out-dir target/unsafe-review-first-pr-smoke
cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review-first-pr-smoke
cargo run --locked -p xtask -- check-pr
git diff --check
```

If a later command fails because source and swarm had already diverged, fix the
backfill in swarm rather than opening another direct implementation PR in
`unsafe-review`.

## Next

After this PR lands, continue 0.2.0 implementation work in swarm. Promote to the
source repo only as explicit, curated, swarm-originated release batches.
