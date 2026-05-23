# comment-plan quality implementation plan

Status: proposed
Owner: pr-comments
Linked spec: ../../docs/specs/UNSAFE-REVIEW-SPEC-0022-pr-commenting-experience.md

## Goal

Make `comment-plan.json` a quiet, checkable, reviewer-useful inline comment
plan without enabling automatic comment posting by default.

## Work item ladder

1. Keep generated comment plans capped at three candidates.
2. Add structured next actions, selection reasons, actionability metadata, and
   candidate trust boundaries to every candidate.
3. Reject duplicate card IDs, duplicate lines, and unsupported classes in the
   artifact verifier.
4. Enforce the hard 220-word body limit in the artifact verifier.
5. Add fixture-backed examples for selected and not-selected cards, with
   `not_selected` reasons in the plan artifact.
6. Document a future trusted poster workflow that consumes verified artifacts
   without rerunning analysis truth. The design lives in
   `docs/ci/TRUSTED_COMMENT_POSTER.md`.

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
