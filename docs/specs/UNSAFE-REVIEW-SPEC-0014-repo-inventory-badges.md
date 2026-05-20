# UNSAFE-REVIEW-SPEC-0014: Repo inventory and badges

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md

## Problem

`unsafe-review` needs a precise, checkable behavior contract for repo inventory and badges.

## Behavior

Repo mode is a static posture snapshot projected from `ReviewCard`s. It reports
repo-scope summary counts, card JSON, Markdown posture reports, advisory policy,
and the static-review trust boundary.

Repo JSON uses this top-level contract:

```text
schema_version
tool
scope = repo
mode = repo
policy = advisory
trust_boundary
root
summary
cards
```

The `summary` object must include:

```text
rust_files
changed_rust_files
unsafe_sites
cards
open_actionable_gaps
contract_missing
guard_missing
guarded_unwitnessed
unsafe_unreached
requires_loom
miri_unsupported
static_unknown
```

The `cards` array must reuse the canonical `ReviewCard` JSON shape. Repo JSON
must not reclassify cards, invent a separate evidence model, or summarize raw
unsafe usage as safety posture.

Badge JSON is a small open-gap summary for shields-compatible consumers:

- `unsafe-review.json` reports `<n> open gaps`
- `unsafe-review-plus.json` reports contract, guard, and current
  guarded-unwitnessed summary counts

Badges count unresolved review evidence. They never claim the repository is
safe, UB-free, Miri-clean, or policy-compliant.

Outcome comparison reads two saved `unsafe-review --format json` snapshots and
reports card identity deltas:

- `new`
- `resolved`
- `improved`
- `regressed`
- `unchanged`

Outcome comparison must compare existing card identity, class actionability,
missing-evidence counts, and saved witness receipt strength from the supplied
snapshots. It must not rerun analysis, run witnesses, post policy decisions, or
claim repository safety.

Baseline-known items, suppressions, and no-new-debt policy promotion remain
separate policy surfaces and are not part of badge proof.

## Non-goals

- no soundness claim
- no hidden blocking unless policy mode explicitly enables it
- no duplicate truth outside this spec and linked policy files
- no safety badge
- no baseline, suppression, or no-new-debt policy in the badge JSON
- no outcome comparison without saved snapshot inputs

## Required evidence

- fixture-backed examples for positive and negative cases
- JSON output contract coverage
- CLI e2e coverage for repo JSON and badge JSON
- CLI e2e coverage for outcome comparison JSON/Markdown
- policy documentation when behavior is configurable

## Acceptance examples

- Repo JSON for a fixture reports `scope = repo`, advisory policy, open-gap
  counts, cards, and the trust boundary.
- Repo Markdown for a fixture reports repo posture, summary counts, top card
  classes, operation families, witness routes, cards, and the trust boundary.
- Badge JSON for a fixture reports open unsafe-review gaps rather than raw
  unsafe count or safe/unsafe status.
- Outcome comparison between a no-card snapshot and a one-card snapshot reports
  one `new` card and preserves the static-review trust boundary.
- Outcome JSON includes `schema_version`, deterministic `before_id` and
  `after_id` snapshot fingerprints, grouped `cards.new`, `cards.resolved`,
  `cards.improved`, `cards.regressed`, and `cards.unchanged` arrays, explicit
  limitations, and the trust boundary.
- Each outcome card includes a reason that explains the snapshot movement, such
  as a class change, missing-evidence count change, witness receipt strength
  movement, new card, or resolved card.
- When the saved snapshots include ReviewCard context, each outcome card state
  preserves the card site, operation family, and hazards. Outcome comparison
  uses that context for reporting only; it does not reclassify cards.
- If evidence is not knowable statically, repo output and badges count the
  card state instead of overclaiming.

## CI proof

```bash
cargo xtask check-pr
cargo test --workspace
cargo test -p unsafe-review --test e2e repo_inventory_and_badges_count_open_gaps_without_safety_claim
cargo test -p unsafe-review --test e2e outcome_compares_existing_json_snapshots_without_safety_claim
```

## Promotion rule

Move from experimental to usable alpha only after fixture, golden, and dogfood receipts exist.
