# Legacy goals archive

This directory is a read-only parity snapshot retained for the `.rails` to
cargo-allow migration. It is not a current execution front door, scheduler,
portfolio, controller goal, or source of task state.

Do not begin work by reading `.rails/goals/active.toml`. Current work starts
from the user-selected live GitHub issue or PR and its accepted issue/work-spec
contract. Durable graph context lives under `.allow`; the compatibility charter
at `.allow/goals/active.toml` is also not a task selector.

Historical work item statuses found here (`ready`, `active`, `blocked`, `done`,
`superseded`) describe archived migration-era records only. They must not be
projected into runtime goal state or used to stop current work.

Use:

1. the selected live GitHub issue or PR;
2. its accepted issue/work-spec contract;
3. linked plan/spec/ADR/proposal evidence;
4. current code, checks, and repository policy.

Retirement of this archive and `xtask check-goals` remains governed by #1877.
