# Deferred editor extension surface

This project plans a VS Code/Open VSX extension surface that consumes
`unsafe-review` saved artifacts (especially saved LSP projections) without
changing source files.

## Planned scope

- Read-only diagnostics and hovers from saved `lsp.json` projection output.
- Reviewer-first navigation for cards, contracts, guards, and witness routes.
- Explicit trust-boundary messaging in the UI (advisory evidence, not proof).

## Non-goals (current)

- No automatic code edits.
- No witness execution.
- No default blocking policy.

## Publication status

As of May 19, 2026, there is no published VS Marketplace or Open VSX listing
for `unsafe-review`. README badges intentionally use `planned` wording until
publication exists.
