# Editor extension surface status

This project ships a VS Code / Open VSX extension surface that consumes
`unsafe-review` saved artifacts (specifically `target/unsafe-review/lsp.json`)
without changing source files.

## Implemented today

- **Saved-`lsp.json` viewer MVP** lives under `editors/vscode/`.
  Scope and non-goals are documented in
  [docs/editor/extension-mvp.md](../editor/extension-mvp.md). The MVP
  publishes diagnostics, hovers, and command-only copy/open actions from
  the saved bundle. It never starts a live LSP server, never runs any
  analyzer subprocess, never edits source, never runs witnesses, and
  never posts PR comments.
- **Packaging CI** runs on every relevant PR through
  `.github/workflows/editor-extension.yml` (advisory lane
  `editor-extension-packaging`). It compiles the extension, runs unit
  tests, and produces a `unsafe-review-vscode.vsix` workflow artifact for
  smoke installs via `code --install-extension`.
- **Manual publish workflow** is ready at
  `.github/workflows/editor-publish.yml`. It is `workflow_dispatch`-only,
  must be dispatched from `main`, defaults to `dry_run=true`, and refuses
  to publish unless a publish-target boolean is set and the matching
  secret is configured. Pre-publication checklist lives in
  [docs/editor/marketplace-publication.md](../editor/marketplace-publication.md).

## Deferred

- **Live `unsafe-review lsp` client wiring** inside the extension is
  still deferred behind the `UNSAFE-REVIEW-SPEC-0018` hardening gate.
  The MVP saved-bundle path explicitly does not consume a live server.

## Publication status

There is no published VS Marketplace or Open VSX listing yet. The
README's `VS Code extension planned` and `Open VSX planned` badges
intentionally stay in `planned` wording until the owner provisions the
publisher accounts, Eclipse / Open VSX namespace, and `VSCE_PAT` /
`OVSX_PAT` repository secrets, and runs
`gh workflow run editor-publish.yml --ref main -f version=<x.y.z>`.

After a successful publication, follow the "After publication" section
of `docs/editor/marketplace-publication.md` to flip the README badges to
`installs` / `downloads` shields and update this doc.

## Non-goals (current)

- No automatic code edits.
- No witness execution.
- No default blocking policy.
- No marketplace publication on PR merge or push.
