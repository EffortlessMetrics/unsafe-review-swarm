# UNSAFE-REVIEW-SPEC-0022: PR commenting experience

Status: accepted
Owner: core/spec
Created: 2026-05-21
Linked specs:
- [UNSAFE-REVIEW-SPEC-0002: ReviewCard schema](UNSAFE-REVIEW-SPEC-0002-review-card-schema.md)
- [UNSAFE-REVIEW-SPEC-0011: PR and CI output](UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md)
- [UNSAFE-REVIEW-SPEC-0012: LSP and editor projection](UNSAFE-REVIEW-SPEC-0012-lsp-editor-projection.md)
- [UNSAFE-REVIEW-SPEC-0013: Agent packets](UNSAFE-REVIEW-SPEC-0013-agent-packets.md)
- [UNSAFE-REVIEW-SPEC-0019: First-run cockpit](UNSAFE-REVIEW-SPEC-0019-first-run-cockpit.md)
Linked plan: ../../plans/post-0.2.0/comment-plan-quality.md
Support-tier impact: ../../docs/status/SUPPORT_TIERS.md
Policy impact:
- ../../policy/ci-lane-whitelist.toml
- ../../policy/doc-artifacts.toml

## 1. Purpose

`unsafe-review` PR comments must help reviewers act on changed unsafe Rust without turning the tool into a noisy bot.

The PR commenting experience is not a full code review. It is a focused projection from existing `ReviewCard`s.

A good comment answers:

- What changed?
- What unsafe obligation is missing evidence?
- What should the author add?
- Which witness route is relevant?
- What is unsafe-review not claiming?

Every comment must be focused, relevant, actionable, bounded, advisory, and ReviewCard-derived.

## 2. Core doctrine

The commenting surface has three layers:

- `pr-summary.md`: broad PR-level reviewer panel.
- `comment-plan.json`: verified, plan-only inline comment candidates.
- Posted comments: future trusted workflow only; disabled by default.

For v0.x, `comment-plan.json` is the supported inline surface. No workflow posts comments by default. A future poster must consume `comment-plan.json` and must not rerun analysis or create separate finding truth.

## 3. Non-goals

The PR commenting experience does not provide automatic posting by default, blocking policy, source edits, suppression insertion, witness execution, or proof claims (memory-safety, UB-free, Miri-clean, site-execution, calibrated precision/recall).

The goal is reviewer leverage, not commenting on every card.

## 4. `comment-plan.json` contract

`comment-plan.json` is the canonical PR comment artifact.

- Mode is plan-only and policy is advisory.
- Default candidate count is 0-3; hard max is 3.
- `summary` records the bounded review budget:
  `selected_count`, `not_selected_count`, `budget`, and `reason`.
- Cards that are present but not selected for inline comments must appear in
  `not_selected[]` with a reason.
- Every ReviewCard must be represented once, either in `comments[]` or
  `not_selected[]`.
- Candidate locations must be renderable (`path` + one-based non-zero `line`).
- Candidate bodies must include a trust boundary statement.

Each selected candidate includes required fields:

- `card_id`, `path`, `line`
- `changed_line`
- `operation`, `operation_family`, `class`, `priority`, `confidence`
- `next_action`, `witness_routes`, `verify_commands`
- `selection_reason`, `actionability`, `relevance`
- `body`, `trust_boundary`

Each `not_selected` entry includes:

- `card_id`, `path`, `line`
- `changed_line`
- `operation`, `operation_family`, `class`, `priority`, `confidence`
- `next_action`
- `actionability`, `relevance`
- `reason`

## 5. Selection rules

Selection is sparse and changed-line only:

- max 3 comments
- one comment per card
- one comment per line unless explicitly justified
- one comment per operation family plus missing-obligation set by default
- no duplicate card IDs

Never select suppressed, `baseline_known`, `static_unknown`, or
`operation_family: "unknown"` cards. Prefer actionable changed unsafe
operations that name specific missing evidence and a concrete next action.
Additional cards in an already-selected operation family and missing-obligation
set remain in `not_selected[]` with reason `covered by selected
family/obligation sibling`; this preserves the review budget without hiding the
underlying ReviewCards.
Cards that are present in the ReviewCard set but lack a changed-line anchor
must stay out of `comments[]` with reason `outside changed hunk`; this keeps
future inline comments tied to PR review context without hiding the card from
the artifact bundle.

Selected `selection_reason` values use this closed vocabulary:

- `actionable high-confidence review card`
- `actionable high-priority review card`

`not_selected[].reason` values use this closed vocabulary:

- `outside changed hunk`
- `class not eligible for inline comments`
- `operation family unknown`
- `confidence below inline comment threshold`
- `priority/confidence below inline comment threshold`
- `covered by selected family/obligation sibling`
- `comment-plan max of three candidates reached`
- `not selected by current inline comment policy`

## 6. Relevance and actionability

Candidates carry transparent relevance metadata (for reviewer-noise control, not policy) and actionability taxonomy.

Relevance values:

- `high` — high priority and high confidence; the reviewer should see this comment first.
- `medium` — one of priority or confidence is high while the other is not low; appears in the inline plan.
- `low` — low or unknown confidence, or neither priority nor confidence is high; never inline-selected.

Relevance is derived from the same priority + confidence signal that drives selection. It does not introduce a new analyzer truth or a policy gate; it only exposes the existing ranking so reviewers can sort the inline plan without re-deriving it.

Actionability values:

- `specific_guard_missing`
- `specific_contract_missing`
- `specific_witness_missing`
- `specific_receipt_missing`
- `specific_reach_missing`
- `specific_repair_available`
- `human_review_only`
- `not_actionable`

Normally eligible for inline comments: `specific_guard_missing`, `specific_contract_missing`, `specific_witness_missing`, `specific_repair_available`.

The summary reason is review-noise metadata. It does not create a policy gate
and does not change ReviewCard truth.

## 7. Comment body contract

Bodies are reviewer notes, not scanner dumps.

Required sections:

- heading
- why this matters
- missing evidence
- what resolves this
- witness route (if useful)
- trust boundary

The body must project the same ReviewCard class, operation, operation family,
missing-evidence summary, next action, first witness route, and first verify
command as the structured comment entry. The structured fields remain the
machine contract; the body is the reviewer-facing rendering of that same card.

Length budget: recommended <=140 words, hard max 220 words.

Forbidden patterns include overclaims (`"This PR is unsafe."`, `"Verified."`, `"Miri-clean."`), generic non-actionable comments, and large internal dumps.

## 8. Summary comment model (future)

A future trusted workflow may post a sticky/idempotent summary comment, but posting is not part of default 0.2.x behavior.

## 9. Posting architecture (future)

Required split model:

1. Untrusted `pull_request` analysis workflow (read-only) generates and verifies artifacts.
2. Separate trusted poster workflow (write token) downloads artifacts, re-verifies `comment-plan.json`, and posts/updates comments.

The poster must not rerun analysis truth, run witness tools, edit source, or post beyond the validated plan.

The detailed trusted-poster design is in
[docs/ci/TRUSTED_COMMENT_POSTER.md](../ci/TRUSTED_COMMENT_POSTER.md). That
document is a future-lane contract, not a live workflow.

## 10. Verifier contract

`cargo run --locked -p xtask -- check-first-pr-artifacts <dir>` must reject malformed or policy-violating `comment-plan.json`, including:

- over max comments
- missing required fields
- missing or mismatched `summary` review-budget counts
- invalid/unknown `card_id`
- invalid/unknown `not_selected.card_id`
- a ReviewCard missing from both `comments[]` and `not_selected[]`
- a `not_selected` card that is also present in `comments[]`
- duplicate `card_id` or duplicate `path`/`line` inline anchors
- invalid line/path
- missing `next_action`, `selection_reason`, `actionability`, `relevance`, or
  candidate `trust_boundary`
- planned comments with `changed_line = false`
- `not_selected[]` entries with `changed_line = false` whose reason is not
  `outside changed hunk`
- drift between `not_selected` review context and the referenced ReviewCard
- `relevance` outside the documented set (`high`, `medium`, `low`)
- body text that drifts from the structured ReviewCard projection
- missing trust boundary in body
- body text over 220 words
- forbidden overclaim wording
- forbidden classes (`static_unknown`, `baseline_known`, suppressed)
- forbidden unknown operation-family comments

## 11. Acceptance examples

Representative outcomes:

- changed raw pointer read with missing alignment evidence -> one `guard_missing` candidate with concrete repair and trust boundary.
- `static_unknown`, `operation_family: "unknown"`, `baseline_known`, low-signal witness-only cards, or no changed-line anchor -> no inline comment, with explicit `not_selected` reason.
- malformed overclaim comment text -> verifier failure.

Fixture-backed selected, card-present/not-selected, and no-card examples are in
[docs/ci/COMMENT_PLAN_EXAMPLES.md](../ci/COMMENT_PLAN_EXAMPLES.md).

## 12. CI proof

Minimum proof:

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- check-pr
```

Artifact proof:

```bash
cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --out-dir target/unsafe-review-comment-plan-smoke

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-comment-plan-smoke
```

No-card proof:

```bash
cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/safe_code_no_cards \
  --diff fixtures/safe_code_no_cards/change.diff \
  --out-dir target/unsafe-review-no-card-comment-plan-smoke

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-no-card-comment-plan-smoke
```
