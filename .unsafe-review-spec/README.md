# unsafe-review source-of-truth state

This directory stores repository-owned coordination state for unsafe-review
development and release lanes.

- Namespace index: `.unsafe-review-spec/index.toml`
- Current goal: `.unsafe-review-spec/goals/active.toml`
- Historical goals: `.unsafe-review-spec/goals/archive/`
- Lane trackers: `.unsafe-review-spec/lanes/`
- Agent operating entrypoint: `AGENTS.md`

## Source-of-Truth Rule

Proposal says why. Spec says what. ADR says what decision. Lane says what
sequence. Support tiers say what users may believe. Policy ledgers say what
exceptions exist. Receipts and proof notes say what proved it. Closeouts say
what happened and what remains.

Do not store product runtime output here. Runtime receipts stay under
`.unsafe-review/receipts/`, and generated review artifacts stay in their
documented output locations.

Do not store durable repo operating state in external tool namespaces such as
`.codex/`, `.spec/`, `.claude/`, or `.jules/`. Those directories may exist for
tool/session state, but unsafe-review's durable coordination state belongs in
`.unsafe-review-spec/`, `docs/`, `plans/`, `policy/`, and documented handoff or
status surfaces.

`AGENTS.md` is the agent-facing entrypoint for these rules. Keep it aligned
with this directory when repo operation style changes, but do not move durable
unsafe-review source-of-truth data into agent-local tool directories.
