# Spec system start here

This file is the operator front door for the spec system.

If you are new to the repository, read in this order:

1. The selected GitHub issue and its dependency/disposition metadata
2. `.allow/goals/active.toml` and the cargo-allow worklist/graph (durable
   charter and links; the worklist is diagnostic, not scheduling authority)
3. linked `plans/.../implementation-plan.md` and issue-linked work specification
4. linked spec and proposal (behavior contract and rationale)
5. `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` (lifecycle state and proof posture)

## Start here by job

| Job | Read first | Then |
|---|---|---|
| PR review lane | `UNSAFE-REVIEW-SPEC-0011` | `UNSAFE-REVIEW-SPEC-0019`, selected issue, implementation contract |
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
| Codex/agent execution | selected issue and controller capsule | charter -> linked plan -> linked spec -> linked proposal |

## What this front door answers

- **What is authoritative?** Durable specs and the cargo-allow artifact graph.
- **What is active?** GitHub issue/project/PR metadata and the controller capsule.
- **What proof applies?** The accepted issue/work-spec contract plus linked spec/plan proof sections.
- **What claim may be made?** Support tiers and spec claim-boundary sections.
- **What PR comes next?** The selected unblocked GitHub issue and its dependency/disposition metadata.

## Non-goal

This file does not redefine product behavior. It routes readers to existing source-of-truth artifacts.
