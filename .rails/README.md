# unsafe-review source-of-truth state

This directory stores repository-owned coordination state for unsafe-review
development and release lanes.

- Namespace index: `.rails/index.toml`
- Current goal: `.rails/goals/active.toml`
- Historical goals: `.rails/goals/archive/`
- Lane trackers: `.rails/lanes/`
- Agent operating entrypoint: `AGENTS.md`

## Why `.rails`?

`.rails` is the portable convention for repository-owned source-of-truth
coordination state across this org's Rust repos. The name is deliberate, both
literal and metaphorical: well-designed specs and tests are the *rails* that keep
work on-design and verifiable — directional specs, drift-lock tests, and the
links between them that let any agent or contributor enter cold and stay on
track.

It is chosen over a per-repo `.<repo>-spec` name because it is:

- **Portable** — the same directory name works in every repo, so the operating
  contract and agent roles transfer without renaming.
- **Recognizable** — "Rust on Rails" reads immediately as "this is the
  convention-driven control plane," where `.unsafe-review-spec` reads as an
  ad-hoc per-repo folder.
- **Collision-free** — unused by Rust tooling, and unlike `.spec/` it does not
  clash with tool/session namespaces (see the rule below).

This directory was renamed from `.unsafe-review-spec/`; all repo references,
gates (`check-goals`, `check-doc-artifacts`), CI triggers, and agent contracts
point at `.rails/`.

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
with this directory when repo operation style changes, but do not move durable
unsafe-review source-of-truth data into agent-local tool directories.
