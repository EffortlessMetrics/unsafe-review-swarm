# live LSP server implementation plan

Status: proposed
Owner: editor/lsp
Linked proposal: ../../docs/proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked spec: ../../docs/specs/UNSAFE-REVIEW-SPEC-0018-live-lsp-server.md
Linked ADR: ../../docs/adr/UNSAFE-REVIEW-ADR-0006-live-lsp-server-is-read-only.md

## Work item ladder

1. docs(spec): add live LSP server contract.
2. cli(lsp): add tower-lsp-server stdio skeleton.
3. lsp: publish review-card diagnostics.
4. lsp: show review-card hover evidence.
5. lsp: add read-only review-card actions.
6. lsp: execute bounded review-card commands.
7. xtask: add live LSP smoke test.
8. docs(lsp): record live LSP support boundary.
9. editors(vscode): optional adapter skeleton (later).

## Validation commands

```bash
cargo run --locked -p xtask -- check-docs
cargo run --locked -p xtask -- check-pr
git diff --check
```

## Acceptance rails

- Read-only invariant: no WorkspaceEdit/TextEdit in actions.
- Trust-boundary invariant: no safety/UB-free/Miri-clean claims.
- Witness invariant: commands may return witness commands for copying only; must not execute witness tools.
- Policy invariant: LSP config cannot enable blocking or no-new-debt defaults.
- Unsaved-buffer invariant: `didChange` does not trigger analysis by default.
