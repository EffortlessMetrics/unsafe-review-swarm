# Editor extension MVP

This document scopes the first publishable `unsafe-review` editor extension as
a read-only saved-`lsp.json` viewer.

It is a planning companion to:

- [UNSAFE-REVIEW-SPEC-0012: LSP and editor projection](../specs/UNSAFE-REVIEW-SPEC-0012-lsp-editor-projection.md)
- [UNSAFE-REVIEW-SPEC-0021: VS Code and Open VSX editor extension](../specs/UNSAFE-REVIEW-SPEC-0021-vscode-openvsx-extension.md)
- [docs/editor/saved-lsp-json.md](saved-lsp-json.md)
- [.rails/lanes/editor-extension/tracker.toml](../../.rails/lanes/editor-extension/tracker.toml)
- [.rails/lanes/marketplace-first-hour-ux/tracker.toml](../../.rails/lanes/marketplace-first-hour-ux/tracker.toml)

The MVP path explicitly frees the first publishable extension from the
`UNSAFE-REVIEW-SPEC-0018` live-LSP hardening gate by removing all live
server wiring from the MVP surface.

## Problem

A maintainer who runs `unsafe-review first-pr` gets a saved
`target/unsafe-review/lsp.json` that describes diagnostics, hovers, and
command-only actions for every changed unsafe-adjacent gap. There is no way
to surface that file in VS Code or Open VSX today. The full live-LSP
extension is blocked on `UNSAFE-REVIEW-SPEC-0018` server hardening (git diff
failure handling, locks across await, refresh generations, stale diagnostics,
UTF-16 ranges, stable command objects).

Waiting on the full live extension delays the editor reach the lane needs.

## MVP scope

Ship a publishable extension that consumes the saved bundle only:

- Watch (or manually load) `${workspace}/target/unsafe-review/lsp.json`.
- Publish diagnostics from that file using the saved `diagnostics[]` entries.
- Render hovers from the saved `hovers[]` entries.
- Expose per-card command-only actions from the saved `code_actions[]` payloads:
  - `unsafe-review.copyAgentPacket` — copy the bounded card-scoped repair packet to the clipboard.
  - `unsafe-review.copyWitnessCommand` — copy the witness command to the clipboard.
- Expose bundle-level command-only actions derived from the configured bundle
  path:
  - `unsafe-review.openPrSummary` — open `target/unsafe-review/pr-summary.md` in the editor.
  - `unsafe-review.openWitnessPlan` — open `target/unsafe-review/witness-plan.md` in the editor.
  - `unsafe-review.refreshBundle` — re-read `target/unsafe-review/lsp.json`.
- Status bar / notification on initial load reading the saved `status.boundary` text from the projection.
- Settings:
  - `unsafeReview.bundlePath` (default `target/unsafe-review/lsp.json`),
  - `unsafeReview.autoRefreshOnSave` (default `false`),
  - `unsafeReview.maxDiagnosticsPerFile` (default `200`).

## MVP non-goals

The MVP must not:

- start `unsafe-review lsp` or wire a live LSP client,
- run `unsafe-review` (or any other analyzer) to refresh diagnostics,
- run Miri, `cargo-careful`, sanitizers, Loom, Shuttle, Kani, or Crux,
- create or edit `WorkspaceEdit` / `TextEdit` quick fixes,
- post PR comments or call any GitHub API,
- enable a default blocking policy,
- send telemetry by default,
- bundle a Rust analyzer fork,
- claim memory safety, UB-free status, Miri-clean status, site-execution
  proof, or calibrated precision/recall.

The saved bundle is the only source of analyzer truth. The MVP never invents
a second one.

## Trust boundary surface

Every entry point in the extension that surfaces a finding must repeat the
saved trust boundary in plain text:

```text
Advisory unsafe contract review. Not memory-safety proof, not UB-free
status, not Miri-clean status, and not site-execution proof.
```

The marketplace README, package description, hover footer, and first-load
status bar message must all include the same wording, taken from the saved
projection's `status.boundary` field when present.

## Refresh model

The MVP refresh model is deliberately minimal:

1. On activation, look for `target/unsafe-review/lsp.json`. If missing, show a
   one-time information message pointing at the first-hour guide and exit
   quietly.
2. On `unsafe-review.refreshBundle`, re-read the file and republish.
3. With `unsafeReview.autoRefreshOnSave` enabled, watch the bundle file via
   `workspace.createFileSystemWatcher` and re-read on `onDidChange`.
4. Never invoke a subprocess. The user runs `unsafe-review first-pr`
   themselves, or wires it into a task / CI workflow.

## Diagnostics shape

Diagnostics are taken directly from the saved projection. The MVP must:

- preserve the `card_id` in `Diagnostic.code` (string),
- use the projection's `severity` field (default `Information`),
- preserve `source` as the literal string `unsafe-review`,
- preserve the message exactly as saved (it already includes the obligation,
  missing evidence, and next action),
- preserve the saved range when present and reject (skip) entries whose
  range fields are missing or non-numeric. Line `0` is valid because the saved
  projection uses LSP-style zero-based ranges.

The extension may cap diagnostics per file at `unsafeReview.maxDiagnosticsPerFile`
to keep editor UI responsive on very noisy bundles; capping always discards
extras, never reorders.

## Code actions shape

Code actions are pure clipboard / open-document commands.

- Every code action must use the stable `payload` object from the saved
  projection. Positional `arguments` are accepted as a fallback for backward
  compatibility.
- Code actions must declare `kind = CodeActionKind.Empty` (or a `quickfix`
  alias that is purely informational); they must not modify the document.
- Extension-defined command labels should end with `(copy)` or `(open)` where
  practical so users see immediately that no source edit will happen. Saved
  projection titles may be shown as-is when preserving the bundle text is more
  important.

## Hover shape

Hovers are rendered from the saved `hovers[]` text. The extension must:

- render hovers as Markdown,
- ensure the trust boundary is present as a footer block on every hover,
  reusing the saved hover text when it already includes that footer,
- include the card id in a code span so reviewers can paste it into
  `unsafe-review explain` or `unsafe-review context --json`.

## Packaging non-goals for the MVP

Publication to VS Marketplace and Open VSX is a separate downstream work
item. The MVP packaging path is:

1. `npm --prefix editors/vscode ci`
2. `npm --prefix editors/vscode run compile`
3. `npm --prefix editors/vscode test`
4. `npx @vscode/vsce package --out target/unsafe-review-vscode.vsix`
5. Attach the VSIX to a GitHub Release for `code --install-extension` smoke
   testing.

Marketplace publication only happens after a successful VSIX install round
trip and an explicit `workflow_dispatch` publish workflow. Manifests and the
publisher identifier are scoped to `effortlessmetrics`.

The owner-facing checklist for marketplace publication and the
`workflow_dispatch`-only publish workflow are documented in
[docs/editor/marketplace-publication.md](marketplace-publication.md). That
workflow defaults to `dry_run=true`; publication never happens on PR merge
or push to main.

## Acceptance examples

The MVP is acceptable when:

- A user runs `unsafe-review first-pr --base origin/main` and the extension
  immediately shows diagnostics for the changed unsafe sites.
- Hovering over a diagnostic shows the saved obligation, missing evidence,
  and next action, plus the trust boundary footer.
- `Unsafe Review: Copy Agent Packet (copy)` writes the bounded packet to the
  clipboard with no side effects on the file.
- `Unsafe Review: Refresh Bundle` re-reads the saved file without running
  any subprocess.
- The extension never edits source, never starts a long-running process,
  never opens a network connection, and never produces analyzer output that
  is not present in the saved projection.

## Forward path

After the MVP ships and is installed at least once:

- `extension-lsp-client` in
  `.rails/lanes/editor-extension/tracker.toml` unblocks behind
  the live-LSP hardening gate, adding a `live` mode toggle to the same
  extension.
- The MVP saved-bundle mode remains the default until live mode passes the
  hardening gate.
- The MVP code path stays the fallback when no live server is configured or
  available.
