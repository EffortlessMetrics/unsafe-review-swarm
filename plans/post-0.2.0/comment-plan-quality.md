# comment-plan quality implementation plan

Status: implemented
Owner: pr-comments
Linked spec: ../../docs/specs/UNSAFE-REVIEW-SPEC-0022-pr-commenting-experience.md

## Goal

Make `comment-plan.json` a quiet, checkable, reviewer-useful inline comment
plan without enabling automatic comment posting by default.

## Work item ladder

1. Done: generated comment plans are capped at three candidates.
2. Done: selected and not-selected entries carry structured next actions,
   reason codes, actionability metadata, relevance, and trust boundaries.
3. Done: the artifact verifier rejects duplicate card IDs, duplicate inline
   anchors, unsupported classes, unknown operation-family comments, and card
   coverage drift.
4. Done: the artifact verifier enforces the hard 220-word body limit.
5. Done: fixture-backed examples cover selected, not-selected, and no-card
   shapes, including explicit `not_selected` reasons.
6. Done: `docs/ci/TRUSTED_COMMENT_POSTER.md` documents a future trusted poster
   workflow that consumes verified artifacts without rerunning analysis truth.

## Non-goals

- No automatic comment posting by default.
- No source edits.
- No witness execution.
- No blocking policy.
- No safety, UB-free, Miri-clean, site-execution, or calibrated precision claims.

## Proof commands

```bash
cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --out-dir target/unsafe-review-comment-plan-smoke

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-comment-plan-smoke

cargo run --locked -p xtask -- check-pr
git diff --check
```

## Claim boundary

This plan improves advisory PR comment artifacts only. It does not make
findings authoritative, post comments, run witnesses, or prove unsafe code safe.
