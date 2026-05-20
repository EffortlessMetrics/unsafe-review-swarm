# 0.2.0 implementation plan

## Work item: source-of-truth-scaffold

Status: active
Linked proposal: UNSAFE-REVIEW-PROP-0002
Linked spec: UNSAFE-REVIEW-SPEC-0018
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
git diff --check
```

### Rollback

Revert this commit.

### Claim boundary

This does not prove xtask validators are fully implemented.
