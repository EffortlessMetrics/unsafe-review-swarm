# UNSAFE-REVIEW-SPEC-0021: VS Code and Open VSX editor extension

Status: accepted, partial-runtime
Owner: editor/extension
Created: 2026-05-21
Linked specs:
- UNSAFE-REVIEW-SPEC-0012-lsp-editor-projection.md
- UNSAFE-REVIEW-SPEC-0018-live-lsp-server.md
Linked ADRs:
- UNSAFE-REVIEW-ADR-0006-live-lsp-server-is-read-only.md
Linked lane:
- .unsafe-review-spec/lanes/editor-extension/tracker.toml
Support-tier impact: yes  
Policy impact: no

## Problem

`unsafe-review lsp` exposes ReviewCard diagnostics and hovers over stdio, but
users need an editor adapter that can start the server, surface trust-boundary
messages, expose command-only actions, and prepare publishable VSIX/Open VSX
artifacts without weakening the product boundary.

## Behavior

The extension starts or connects to `unsafe-review lsp`.

LSP client wiring is blocked until the live-server hardening gate in
UNSAFE-REVIEW-SPEC-0018 is satisfied. Until then, this spec is a planning rail,
not a claim that an editor package is ready to ship.

The first publishable extension is the saved-`lsp.json` viewer MVP scoped in
[docs/editor/extension-mvp.md](../editor/extension-mvp.md). The MVP path
explicitly removes live-LSP wiring from its surface, which frees publication
from the UNSAFE-REVIEW-SPEC-0018 hardening gate while leaving this spec as
the long-form contract for the eventual full live extension.

It provides:

- diagnostics from the LSP server,
- ReviewCard hover explanations,
- command palette actions:
  - Unsafe Review: Restart Server
  - Unsafe Review: Refresh
  - Unsafe Review: Show Support Boundary
  - Unsafe Review: Copy Agent Packet
  - Unsafe Review: Copy Witness Command
  - Unsafe Review: Open Related Test
- settings for:
  - path to `unsafe-review`,
  - mode: `repo` or `diff`,
  - base ref,
  - max cards,
  - refresh on save/open.

The extension must not create a second analyzer path.

## Non-goals

- no source edits,
- no automatic fixes,
- no witness execution,
- no receipt creation/import,
- no PR comments,
- no blocking policy,
- no telemetry by default,
- no bundled Rust analyzer fork,
- no safety/UB-free/Miri-clean claim.

## Required evidence

- extension manifest validation,
- TypeScript compile/test,
- VSIX package smoke,
- Open VSX package smoke,
- LSP command smoke against `unsafe-review lsp`,
- marketplace README trust-boundary check,
- no `WorkspaceEdit`/`TextEdit` action test,
- no telemetry/network dependency check.

## Acceptance examples

- Opening a Rust repo starts `unsafe-review lsp`.
- Running `Unsafe Review: Refresh` calls `unsafe-review.refresh`.
- A diagnostic includes a card ID and trust boundary.
- Hover shows missing evidence and next action.
- Copy packet command returns bounded ReviewCard packet.
- Copy witness command copies text only; it does not run the command.
- The README and package metadata say advisory/static evidence, not proof.
