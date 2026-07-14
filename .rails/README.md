# unsafe-review legacy source-of-truth archive

Governance authority moved to [`.allow/goals/active.toml`](../.allow/goals/active.toml)
and [`.allow/artifacts/doc-artifacts.toml`](../.allow/artifacts/doc-artifacts.toml)
under cargo-allow's `spec-system` profile. This directory is retained as a
read-only parity snapshot until the migration closeout; do not add new active
goals or route agents here.

This directory stores the historical coordination state for unsafe-review
development and release lanes.

- Namespace index: `.rails/index.toml`
- Current goal: `.allow/goals/active.toml`
- Historical goals: `.rails/goals/archive/`
- Lane trackers: `.rails/lanes/`
- Agent operating entrypoint: `AGENTS.md`

## Why `.rails`?

`.rails` was the portable convention for repository-owned source-of-truth
coordination state. It is now retained only for migration history and parity
comparison; cargo-allow's `.allow` graph is the current machine-checked source.

It is chosen over a per-repo `.<repo>-spec` name because it is:

- **Portable** — the same directory name works in every repo, so the operating
  contract and agent roles transfer without renaming.
- **Recognizable** — "Rust on Rails" reads immediately as "this is the
  convention-driven control plane," where `.unsafe-review-spec` reads as an
  ad-hoc per-repo folder.
- **Collision-free** — unused by Rust tooling, and unlike `.spec/` it does not
  clash with tool/session namespaces (see the rule below).

This directory was renamed from `.unsafe-review-spec/`. The legacy `check-goals`
and related checks remain during the bounded parity window, but current agent
contracts and active-goal routing point at `.allow/`.

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
`.rails/`, `docs/`, `plans/`, `policy/`, and documented handoff or
status surfaces.

`AGENTS.md` is the agent-facing entrypoint for these rules. Keep it aligned
with `.allow` when repo operation style changes, but do not move durable
unsafe-review source-of-truth data into agent-local tool directories.
