# UNSAFE-REVIEW-ADR-0006: Live LSP server is read-only

Status: proposed
Date: 2026-05-20
Owner: editor/lsp
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked specs:
- ../specs/UNSAFE-REVIEW-SPEC-0012-lsp-editor-projection.md
- ../specs/UNSAFE-REVIEW-SPEC-0018-live-lsp-server.md

## Decision

`unsafe-review lsp` is a read-only projection server. It uses `tower-lsp-server` to expose existing `ReviewCard` evidence as diagnostics, hovers, and command-only actions. It does not edit source, run witnesses, create receipts, mutate policy, post comments, or claim safety.

The server lives in `unsafe-review-cli`. `unsafe-review-core` remains the canonical analysis engine and must not depend on LSP transport types.

## Context

`unsafe-review` already emits saved LSP/editor JSON from `ReviewCard`s. The live server should improve editor ergonomics without creating a second analyzer path or weakening the advisory trust boundary.

## Consequences

Positive:

- editor clients can consume live diagnostics and hovers;
- coding agents can request bounded card packets from the editor surface;
- the server remains cheap and local;
- `ReviewCard` identity remains the source of truth.

Negative:

- no automatic fixes;
- no hidden Miri or witness execution;
- no source-edit quick fixes in v1;
- unsaved-buffer precision is intentionally deferred.

## Alternatives considered

### Source-editing quick fixes

Rejected. Unsafe repair needs human review and stronger proof rails.

### Run witness tools from editor commands

Rejected. Witness execution belongs to explicit CLI/user workflows and receipt import surfaces.

### Put LSP transport in unsafe-review-core

Rejected. Core should expose analysis and neutral DTOs, not editor transport.

### Analyze unsaved buffer content

Deferred. It would create a second analyzer path and complicate card identity.
