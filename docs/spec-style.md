# Spec style and ownership boundaries

The repository keeps a full source-of-truth stack:

```text
roadmap -> proposal -> spec -> ADR -> implementation plan -> PRs -> proof -> support/policy -> closeout
```

The durable control plane for this stack is the cargo-allow spec-system graph
rooted in `.allow/` and linked repository artifacts (for example `docs/`,
`plans/`, and `policy/`).

When contributors refer to source-of-truth "rails" in this repository, they mean
the `.allow/` graph plus its linked-docs control plane. The legacy `.rails/`
tree is a read-only parity archive during the migration window; do not route
new work there or create another editable governance root.

## Durable vs external state

Durable rails in this repository:

- `.allow/` for neutral project charter(s), the linked artifact graph, and
  cargo-allow spec-system metadata.
- `.rails/` as a read-only parity archive during the migration window.
- `docs/` for human-facing specs, proposals, ADRs, and contributor guidance.
- `policy/` for live enforcement ledgers and reference maps.
- `plans/` for PR-sized implementation sequencing.

External or tool-specific state (awareness-only for this lane):

- `.codex/`
- `.spec/`
- `.claude/`
- `.jules/`

These namespaces may coexist, but they are not owned by this repository's source-of-truth contract.

## Document role separation

- Proposals explain **why** work exists.
- Specs define **what** behavior is required.
- ADRs capture durable architecture **decisions**.
- Plans and lane trackers define **how** work is sequenced.
- Proof commands and CI receipts show **what proves it**.
- Closeouts capture **what happened** and what remains.

Do not collapse these roles into one mixed-purpose document.
