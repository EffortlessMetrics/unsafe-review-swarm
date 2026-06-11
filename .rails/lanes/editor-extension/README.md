# editor-extension lane

This lane tracks the VS Code/Open VSX extension work as a read-only editor
adapter over `unsafe-review lsp`.

## Scope

- Extension scaffolding and packaging under `editors/vscode/`.
- LSP client wiring and command-only editor actions.
- Packaging and publication rails for VSIX/Open VSX.

## Boundaries

- No source edits from the extension.
- No witness execution from the extension.
- No policy enforcement or safety claims from the extension.

See `tracker.toml` for status and proof commands.
