# 2026-05-20 - source/swarm reconciliation

Scope: repair `unsafe-review-swarm` as the implementation workbench by making
it include the current `unsafe-review` source-of-record state.

This is a base repair, not a feature lane. The intended repository model is:

```text
unsafe-review-swarm
  workbench / implementation / dogfood / experiments / proof-building

unsafe-review
  public source of record / curated promotions / release / crates.io surface
```

## Reconciliation method

The first attempted method was a direct merge of source `main` into swarm
`main`:

```bash
git merge public/main --no-commit --no-ff --allow-unrelated-histories
```

That merge was rejected for this repair because the repositories did not share
a merge base and the merge produced broad add/add conflicts across workflows,
lockfiles, fixtures, docs, schemas, core output code, CLI code, `xtask`, and
fuzz metadata.

The chosen method is therefore the safer reseed path:

1. Start this branch from `EffortlessMetrics/unsafe-review` `main`.
2. Replay only swarm-only work that must remain part of the workbench.
3. Document source-only absorption, swarm-only preservation, skipped commits,
   and the future sync rule.

Observed heads at the time of reconciliation:

| Repository | Ref | Commit |
|---|---|---|
| `unsafe-review` | `public/main` | `59cc418 policy: explain report classifications (#486)` |
| `unsafe-review-swarm` | `origin/main` | `090bf74 cli: align help trust boundary (#114)` |

The observed commit divergence was:

```text
origin/main...public/main
  swarm-only: 113
  source-only: 424
```

## Source-only PRs absorbed

Because the branch starts from source `main`, these source-side changes are now
present in the swarm workbench by construction.

| Source PR | Behavior | Swarm status | Reason | Tests / proof |
|---|---|---|---|---|
| `#477` | Defines the `0.2.0` public usability release target. | Absorbed from source base | Source is the public release-planning surface. | Covered by source history and this PR validation. |
| `#478` | Adds `check-first-pr-artifacts` and caps comment-plan output at three planned comments. | Absorbed from source base | First-pr bundle verification belongs in the workbench. | Covered by source history and this PR validation. |
| `#479` | Includes saved `lsp.json` in the `first-pr` bundle. | Absorbed from source base | Source behavior is retained. | Covered by source history and this PR validation. |
| `#480` | Polishes `first-pr` terminal summary. | Absorbed from source base | Source behavior is retained. | Covered by source history and this PR validation. |
| `#481` | Standardizes no-card advisory wording. | Absorbed from source base | Source behavior is retained. | Covered by source history and this PR validation. |
| `#482` | Makes `explain` reviewer-first. | Absorbed from source base | Source behavior is retained. | Covered by source history and this PR validation. |
| `#483` | Adds explain examples for common cards. | Absorbed from source base | Source behavior is retained. | Covered by source history and this PR validation. |
| `#484` | Adds `unsafe-review support`. | Absorbed from source base | Source behavior is retained. | Covered by source history and this PR validation. |
| `#485` | Groups witness-plan routes. | Absorbed from source base | Source behavior is retained. | Covered by source history and this PR validation. |
| `#486` | Explains advisory policy report classifications. | Absorbed from source base | Source behavior is retained. | Covered by source history and this PR validation. |

## Swarm-only work preserved or skipped

| Swarm PR / commit | Status | Action | Source promotion target | Reason |
|---|---|---|---|---|
| `#79` / `a0ede71` live LSP scaffold | Preserved | Replayed onto source base | Deferred, later editor lane only | Already merged in swarm; needed before the live-LSP module split can remain in the workbench. |
| `#107` / `f9fc99b` markdown table tests | Skipped / superseded | Not replayed | None | Source removed `markdown_table.rs`; keeping the source deletion is cleaner than resurrecting a removed module for a narrow test-only commit. |
| `#108` / `383a646` live LSP spec, ADR, and implementation plan | Preserved | Replayed onto source base | Deferred, later editor lane only | Useful swarm-side planning for live LSP; not part of the source `0.2.0` release surface. |
| `#109` / `f145cab` live LSP module split | Preserved | Replayed onto source base | Deferred, later editor lane only | Keeps already-merged swarm live-LSP implementation work available for later hardening. |
| `#110` / `646c0f7` first-run cockpit lane spec | Preserved | Replayed onto source base | Curated 0.2.0 promotion candidate after swarm is green | Aligned with the 0.2.0 public-usability lane and should stay swarm-side until promotion. |
| `#111` / `e97a8d7` source first-run UX backfill | Superseded | Not replayed | None | The reseed starts from source main, so the source-side UX changes are already present directly. |
| `#112` / `ab402ce` doctor first-run readiness | Preserved | Replayed onto source base | Curated 0.2.0 promotion candidate after swarm is green | Already merged in swarm and still part of the public-usability lane. |
| `#113` / `153a98f` first-pr trust boundary tightening | Preserved | Replayed onto source base | Curated 0.2.0 promotion candidate after swarm is green | Keeps first-pr terminal/help wording aligned with the advisory boundary. |
| `#114` / `090bf74` help trust boundary alignment | Preserved | Replayed onto source base | Curated 0.2.0 promotion candidate after swarm is green | Keeps help output aligned with the advisory boundary. |

## Direct-source exception record

Some first-run UX work landed directly in `unsafe-review` after
`unsafe-review-swarm` existed. That work is retained because it is useful and
publicly aligned, but the route was wrong.

Future routine implementation must happen in `unsafe-review-swarm` first.
`unsafe-review` should receive only:

- curated promotions from green swarm work,
- release prep,
- publication receipts,
- public package / docs.rs / crates.io metadata,
- urgent published-user hotfixes.

## Standing sync rule

After any direct source PR merges, open a swarm sync PR immediately unless the
source PR was itself a swarm promotion and is already present in swarm.

Every direct source PR should record:

```text
Swarm sync:
- not needed because: <reason>
- or required via: sync(source #NN): <title>
```

This prevents the source repo from silently drifting ahead of the workbench.

## Trust boundary

This reconciliation does not promote `unsafe-review` beyond its current
advisory posture:

- no memory-safety proof,
- no UB-free claim,
- no Miri-clean claim,
- no site-execution claim,
- no witness execution by default,
- no automatic comments,
- no source edits,
- no default blocking policy.

Live LSP remains swarm-side experimental work. It is not part of the source
`0.2.0` release target until the later editor lane explicitly hardens and
promotes it.

## Validation

The following commands passed for this reconciliation PR:

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- check-pr
cargo run --locked -p unsafe-review -- first-pr --root fixtures/raw_pointer_alignment --diff fixtures/raw_pointer_alignment/change.diff --out-dir target/unsafe-review-first-pr-smoke
cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review-first-pr-smoke
git diff --check
```

Observed summaries:

- workspace tests: `462 passed`,
- `check-pr`: ok with docs, policy, support tiers, `217 fixtures`,
  `215 calibration cases`, `30 dogfood targets`, `7 repositories`, and fuzz
  metadata checks,
- `first-pr` smoke wrote `cards.json`, `pr-summary.md`, `cards.sarif`,
  `comment-plan.json`, `witness-plan.md`, and `lsp.json`,
- `check-first-pr-artifacts`: ok for `target/unsafe-review-first-pr-smoke`.
