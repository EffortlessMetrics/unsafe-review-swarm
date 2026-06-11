# Public product surfaces lane implementation plan

## Goal

Make first-contact public surfaces converge on one product truth and trust
boundary while preserving `ReviewCard` as the canonical unit.

## Sequencing

1. Inventory public surfaces and normalize source-of-truth wording.
2. Add/expand `xtask check-public-surfaces` to enforce boundary language.
3. Verify package and docs entry points (`cargo package --list`, CLI help).
4. Record publication evidence and drift follow-ups in closeout.

## Exit criteria

- Public metadata/docs/help surfaces converge on one trust boundary.
- Checker coverage exists for recurring drift vectors.
- No new claim exceeds advisory v0.x posture.
