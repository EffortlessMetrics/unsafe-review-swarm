# Spec style and ownership boundaries

The repository keeps a full source-of-truth stack:

```text
roadmap -> proposal -> spec -> ADR -> implementation plan -> PRs -> proof -> support/policy -> closeout
```

The durable control plane for this stack is repo-owned state rooted in `.rails/` and linked repository artifacts (for example `docs/`, `plans/`, and `policy/`).

When contributors refer to source-of-truth "rails" in this repository, they mean
this existing `.rails/` plus linked-docs control plane. Do not add a
parallel durable root such as `.rails/` unless a future accepted spec changes the
namespace.

## Durable vs external state

Durable rails in this repository:

- `.rails/` for active lane coordination metadata.
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
