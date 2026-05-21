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

## First scaffold hardening gate

Do not promote the first live LSP scaffold out of swarm until the server proves
these operational details:

- `git diff` failures are logged and do not silently become a clean repo scan.
- `spawn_blocking` join errors and analyzer errors are logged.
- refresh failures clear stale diagnostics or mark status stale.
- refresh publishing does not hold state locks across `.await`.
- refresh generations prevent stale analysis results from publishing.
- diagnostic ranges use UTF-16 character width.
- hover selection is based on URI and cursor position, not the first card.
- code actions are card-scoped and command-only.
- execute-command arguments use stable object payloads with `card_id`.
- dependency and lockfile changes are limited to the live server surface.

Minimum tests before merging the scaffold:

```text
initialize_returns_read_only_capabilities
diagnostic_for_card_carries_card_id_and_trust_boundary
diagnostic_range_uses_utf16_width
hover_selects_card_at_cursor
code_actions_are_command_only
execute_collect_agent_packet_returns_packet_for_card
execute_unknown_command_returns_none
refresh_failure_clears_stale_diagnostics
did_change_does_not_trigger_analysis_by_default
```

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
