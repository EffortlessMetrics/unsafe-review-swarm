# UNSAFE-REVIEW-ADR-0007: Editor extension is a thin read-only adapter

Status: proposed
Date: 2026-05-21
Owner: editor/extension
Linked specs:
- ../specs/UNSAFE-REVIEW-SPEC-0018-live-lsp-server.md
- ../specs/UNSAFE-REVIEW-SPEC-0021-vscode-openvsx-extension.md

## Decision

The VS Code/Open VSX extension is a thin adapter over `unsafe-review lsp`.

It does not analyze Rust source itself. It does not parse `ReviewCard`s except
through LSP payloads. It does not run witnesses, edit source, create receipts,
post comments, or enforce policy.

## Consequences

Positive:

- one analyzer truth,
- easier marketplace trust story,
- lower extension supply-chain risk,
- same behavior in VS Code and Open VSX-compatible editors.

Negative:

- users must have `unsafe-review` installed or configure its path,
- no automatic fixes in the initial v0.x adapter,
- unsaved-buffer analysis stays deferred.
