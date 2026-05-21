# unsafe-review

Packaging scaffold for the future read-only unsafe-review editor adapter.

This scaffold does not yet start `unsafe-review lsp`, publish diagnostics, render
hovers, run commands, execute witnesses, edit source, or enforce policy. It
exists to keep the VS Code/Open VSX package metadata, build output, and trust
boundary ready for the later LSP-client lane.

The extension lane remains advisory:

- ReviewCard remains the product truth.
- Future diagnostics and hovers must come from `unsafe-review lsp`.
- Future actions must be command-only.
- No WorkspaceEdit/TextEdit actions.
- No witness execution.
- No source edits.
- No automatic comments.
- No default blocking policy.
- No memory-safety, UB-free, or Miri-clean claim.

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
