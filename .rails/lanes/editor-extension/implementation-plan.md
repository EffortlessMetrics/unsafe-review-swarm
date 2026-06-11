# Editor extension implementation plan

This plan sequences a thin VS Code/Open VSX adapter over `unsafe-review lsp`.

1. Establish source-of-truth rail docs (spec + ADR + lane tracker).
2. Satisfy the live-LSP hardening gate from SPEC-0018.
3. Scaffold `editors/vscode/` with trust-boundary wording and packaging metadata.
4. Wire LSP client startup and refresh/restart/support-boundary commands.
5. Add command-only actions for packet/witness/test helpers.
6. Add packaging-only CI and later manual publication workflow.
7. Update support-tier documentation after package CI proves the surface.

All milestones remain advisory-first and read-only.
