# unsafe-review (VS Code / Open VSX)

Read-only viewer for the saved `unsafe-review` PR review bundle.

The extension loads `target/unsafe-review/lsp.json` produced by
`unsafe-review first-pr` and publishes the saved diagnostics, hovers, and
command-only copy/open actions inside the editor. It never starts an LSP
server, never invokes any subprocess, never edits source, never executes
witnesses, and never posts PR comments.

## Trust boundary

Advisory unsafe contract review. Not memory-safety proof, not UB-free
status, not Miri-clean status, and not site-execution proof.

`unsafe-review` finds unsafe Rust changes missing a safety contract, guard,
test, or witness. It does not prove the code free of UB.

## What the MVP does

- Loads `${workspace}/target/unsafe-review/lsp.json` on activation and on
  request.
- Publishes diagnostics from the saved `diagnostics[]` entries.
- Renders hovers from the saved `hovers[]` entries, ensuring the trust boundary
  is present as a footer.
- Registers per-card command-only actions from the saved `code_actions[]` and
  bundle-level open/refresh commands:
  - **Unsafe Review: Refresh Bundle** — re-read the bundle from disk.
  - **Unsafe Review: Open PR Summary (open)** — open
    `target/unsafe-review/pr-summary.md` in the editor.
  - **Unsafe Review: Open Witness Plan (open)** — open
    `target/unsafe-review/witness-plan.md` in the editor.
  - **Unsafe Review: Copy Agent Packet Command (copy)** — copy
    `unsafe-review context <card-id> --json` to the clipboard.
  - **Unsafe Review: Copy Witness Command (copy)** — copy the saved
    witness command (e.g. `cargo +nightly miri test ...`) to the
    clipboard.
- Watches the bundle file with a `FileSystemWatcher`. With
  `unsafeReview.autoRefreshOnSave` enabled, refresh is automatic.

## What the MVP does not do

- Never starts `unsafe-review lsp` or any other process.
- Never runs `unsafe-review`, Miri, `cargo-careful`, sanitizers, Loom,
  Shuttle, Kani, or Crux.
- Never creates `WorkspaceEdit` / `TextEdit` quick fixes; the only
  registered code-action kind is `Empty`.
- Never posts PR comments or calls any GitHub API.
- Never sends telemetry.
- Never invents diagnostics, hovers, or code actions outside the saved
  bundle.

See [docs/editor/extension-mvp.md](../../docs/editor/extension-mvp.md) for
the full MVP scope and non-goals, and
[UNSAFE-REVIEW-SPEC-0021](../../docs/specs/UNSAFE-REVIEW-SPEC-0021-vscode-openvsx-extension.md)
for the long-form contract for the eventual live-LSP extension.

## Settings

| Setting | Default | Notes |
|---|---|---|
| `unsafeReview.bundlePath` | `target/unsafe-review/lsp.json` | Workspace-relative path to the saved bundle. |
| `unsafeReview.autoRefreshOnSave` | `false` | Re-read the bundle when it changes on disk. |
| `unsafeReview.maxDiagnosticsPerFile` | `200` | Discards extras for editor UI responsiveness. The full bundle remains on disk. |

## Refresh flow

```bash
# In a terminal
unsafe-review first-pr --base origin/main

# In VS Code / Open VSX
# - the bundle status bar updates automatically if autoRefreshOnSave is on,
# - otherwise: command palette -> Unsafe Review: Refresh Bundle (or click the
#   status bar item).
```

## Local checks

```bash
npm ci
npm run compile
npm test
npx @vscode/vsce package --out ../../target/unsafe-review-vscode.vsix
```

## Support

See [SUPPORT.md](./SUPPORT.md).

> Note: This lane intentionally omits the extension icon binary because this
> review surface does not support binary files; publication lanes should add a
> compliant PNG icon before store submission.
