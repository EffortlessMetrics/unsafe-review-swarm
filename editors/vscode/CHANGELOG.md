# Changelog

## 0.1.0

- Saved-`lsp.json` viewer MVP: loads `target/unsafe-review/lsp.json` from the
  active workspace folder and publishes diagnostics, hovers, and command-only
  copy/open actions.
- Status bar item reports bundle state and offers a click-to-refresh action.
- File watcher refreshes on bundle create / delete; `autoRefreshOnSave`
  setting opts in to refresh on bundle change.
- Settings: `unsafeReview.bundlePath`, `unsafeReview.autoRefreshOnSave`,
  `unsafeReview.maxDiagnosticsPerFile`.
- Commands: Refresh Bundle, Open PR Summary, Open Witness Plan, Copy Agent
  Packet Command, Copy Witness Command. All actions are `Empty` kind; no
  `WorkspaceEdit` / `TextEdit` is ever issued.
- Unit tests for the bundle parser (no `vscode` mock required) run via
  `node --test`.
- Non-goals (unchanged): no live LSP server, no analyzer subprocess
  invocation, no witness execution, no source edits, no PR comment posting,
  no telemetry, no memory-safety / UB-free / Miri-clean / site-execution
  claim.

## 0.0.1

- Initial packaging-only scaffold for VS Code/Open VSX publishing lanes.
- No runtime LSP client, diagnostics, hovers, commands, source edits, witness execution, or policy enforcement is included yet.
- Removed binary icon asset from this PR because binary files are not supported in this review lane; icon will be added in a follow-up publication patch.
