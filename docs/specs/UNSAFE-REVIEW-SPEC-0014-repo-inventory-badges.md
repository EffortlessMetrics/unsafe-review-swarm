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

Repo mode supports bounded file selection before analysis. `--include` and
`--exclude` are repeatable glob filters over root-relative Rust paths. Empty
include filters mean all discovered Rust files are eligible before excludes.
Repo discovery respects gitignore files by default, supports an explicit
opt-out for ignored Rust files, and skips common non-review trees by default:
`.git`, `.github`, `.unsafe-review*`, `target`, `node_modules`, `vendor`,
`build`, `dist`, and any directory named `generated`.

`--list-files` is a dry run over the same discovery and filtering pipeline. It
prints the selected root-relative Rust files and exits without creating
ReviewCards or running witness tools. `--max-files` truncates the selected
file list after deterministic ordering and applies to both dry-run listing and
repo analysis input.

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

Badge JSON is a small serde-backed open-gap summary for Shields-compatible
consumers. Endpoint JSON must keep `schemaVersion = 1` for Shields while any
additional repo-contract fields remain advisory projection metadata:

- `unsafe-review.json` reports the numeric open-gap count as `<n>`
- `unsafe-review-plus.json` reports the numeric repair-plus-quality count as
  `<open_actionable_gaps + contract_missing + guard_missing + guarded_unwitnessed>`
  and exposes the `contract_missing`, `guard_missing`, and
  `guarded_unwitnessed` components in its `counts` object so the aggregate is
  auditable without re-running analysis.

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

Outcome comparison also includes a compact `reviewer_delta` front panel derived
from the same grouped card outcomes. It reports new/resolved/improved/regressed
counts, receipt-strength movement, and top remaining gaps from the after
snapshot. It does not introduce another classification path.

Baseline-known items, suppressions, and no-new-debt policy promotion remain
separate policy surfaces and are not part of badge proof.

## Projection contract

Repo posture and badges are ReviewCard projections. They summarize unresolved
unsafe-review evidence gaps; they do not count raw unsafe usage as repository
posture and do not certify repository safety.

Badge meanings are fixed:

- `unsafe-review`: open unsafe-review gap count
- `unsafe-review+`: open unsafe-review gap count plus missing-or-weak evidence
  findings, with component counts for contract-missing, guard-missing, and
  guarded-unwitnessed evidence quality signals

Badges must never imply that the repo is sound, memory-safe, UB-free,
Miri-clean, verified, all clear, policy-ready, or that any unsafe site executed.
If a badge endpoint cannot be generated or verified, the public badge row must
be withheld or marked planned rather than inferred from another surface.
Repository checks must reject checked-in endpoint JSON that no longer matches
the current `unsafe-review badges` repo projection.

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
- CLI e2e coverage for repo file-selection dry runs
- CLI e2e coverage for outcome comparison JSON/Markdown
- policy documentation when behavior is configurable

## Acceptance examples

- Repo JSON for a fixture reports `scope = repo`, advisory policy, open-gap
  counts, cards, and the trust boundary.
- Repo file-list dry runs honor include/exclude filters, gitignore defaults,
  large-repo default skips, and max-file truncation without analyzing files.
- Repo Markdown for a fixture reports repo posture, summary counts, top card
  classes, operation families, witness routes, cards with direct `path:line`
  source locations, concrete operation expressions and next actions, and the
  trust boundary.
- Badge JSON for a fixture reports open unsafe-review gaps rather than raw
  unsafe count or safe/unsafe status.
- The `unsafe-review+` badge message equals `unsuppressed_review_gaps` plus the
  `evidence_quality_contract_missing`, `evidence_quality_guard_missing`, and
  `evidence_quality_guarded_unwitnessed` component counts.
- Outcome comparison between a no-card snapshot and a one-card snapshot reports
  one `new` card and preserves the static-review trust boundary.
- Outcome JSON includes `schema_version`, deterministic `before_id` and
  `after_id` snapshot fingerprints, grouped `cards.new`, `cards.resolved`,
  `cards.improved`, `cards.regressed`, and `cards.unchanged` arrays, explicit
  limitations, and the trust boundary.
- Outcome JSON includes `reviewer_delta` with compact reviewer counts,
  receipt movement, and top remaining gaps projected from the same outcome
  cards.
- Each outcome card includes a reason that explains the snapshot movement, such
  as a class change, missing-evidence count change, witness receipt strength
  movement, new card, or resolved card.
- Outcome card states include saved ReviewCard operation expression, operation
  family, and next action when present in the input snapshots, without changing
  outcome classification.
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
