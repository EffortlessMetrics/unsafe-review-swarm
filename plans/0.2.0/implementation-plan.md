# 0.2.0 implementation plan

## Work item: source-of-truth-scaffold

Status: active
Linked proposal: UNSAFE-REVIEW-PROP-0002
Linked spec: UNSAFE-REVIEW-SPEC-0020
Linked ADR: none
Blocks: doc-artifact-ledger
Blocked by: none
Branch: docs/source-of-truth-stack
Issue: none
PR: TBD

### Goal

Add baseline artifact taxonomy, templates, goals manifest, and policy placeholders.

### Production delta

Repository documentation, policy ledgers, and CI metadata.

### Non-goals

No runtime crate behavior changes.

### Acceptance

Required paths exist and are cross-linked with stable IDs.

### Proof commands

```bash
cargo run --locked -p xtask -- check-doc-artifacts
cargo run --locked -p xtask -- check-goals
cargo run --locked -p xtask -- check-package-boundary
cargo run --locked -p xtask -- check-ci-lanes
git diff --check
```

### Rollback

Revert this commit.

### Claim boundary

This proves only the source-of-truth scaffolding links and ledgers. It does not
prove unsafe-review runtime analysis behavior.
