# Spec system start here

This file is the operator front door for the spec system.

If you are new to the repository, read in this order:

1. `.unsafe-review-spec/goals/active.toml` (what is active now)
2. linked `plans/.../implementation-plan.md` (what PR-sized step is next)
3. linked spec and proposal (behavior contract and rationale)
4. `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` (lifecycle state and proof posture)

## Start here by job

| Job | Read first | Then |
|---|---|---|
| PR review lane | `UNSAFE-REVIEW-SPEC-0011` | `UNSAFE-REVIEW-SPEC-0019`, active goal, implementation plan |
| Analyzer evidence change | `UNSAFE-REVIEW-SPEC-0004`, `0005`, `0006` | operation-family appendix, `UNSAFE-REVIEW-SPEC-0016` |
| Witness routing/receipts | `UNSAFE-REVIEW-SPEC-0008`, `0009` | `UNSAFE-REVIEW-SPEC-0011`, `UNSAFE-REVIEW-SPEC-0019` |
| Manual candidate ledger | `UNSAFE-REVIEW-SPEC-0027` | `UNSAFE-REVIEW-SPEC-0002`, `0008`, `0009`, `0013` |
| First-run UX / first-pr cockpit | `UNSAFE-REVIEW-SPEC-0019` | `UNSAFE-REVIEW-SPEC-0011`, `0012`, `0013` |
| LSP / IDE projection | `UNSAFE-REVIEW-SPEC-0012` | `UNSAFE-REVIEW-SPEC-0013`, `UNSAFE-REVIEW-SPEC-0019` |
| Agent packet projection | `UNSAFE-REVIEW-SPEC-0013` | `UNSAFE-REVIEW-SPEC-0006`, `0012`, `0019` |
| Inventory, policy, badges | `UNSAFE-REVIEW-SPEC-0010`, `0014` | support tiers, policy ledgers |
| Ease of use / adoption surfaces | `UNSAFE-REVIEW-SPEC-0028` | `UNSAFE-REVIEW-SPEC-0029` coverage model, `0030` baseline movement |
| Adoption surface build (badge/comments/LLM/manifest/repo) | `UNSAFE-REVIEW-SPEC-0028` | `0031` badge, `0032` comment-plan, `0033` LLM packet, `0034` ub-review manifest, `0035` repo-scan |
| Sibling-tool interop / cross-pollination | `docs/interop/sibling-tools.md` | `UNSAFE-REVIEW-SPEC-0028`, `0034` |
| Release prep and publication evidence | `UNSAFE-REVIEW-SPEC-0015`, `0016`, `0019`, `0020` | latest closeout in `docs/handoffs/` |
| Source-of-truth stack maintenance | `UNSAFE-REVIEW-SPEC-0020` | doc artifacts ledger, goals manifest, plan links |
| Codex/agent execution | active goal manifest | linked plan -> linked spec -> linked proposal |

## What this front door answers

- **What is authoritative?** Active goal + linked plan/spec/proposal chain.
- **What is active?** `status = "active"` lane in active goals.
- **What proof applies?** The commands listed in the active work item plus spec/plan proof sections.
- **What claim may be made?** Support tiers and spec claim-boundary sections.
- **What PR comes next?** The next row in the linked implementation plan.

## Non-goal

This file does not redefine product behavior. It routes readers to existing source-of-truth artifacts.
