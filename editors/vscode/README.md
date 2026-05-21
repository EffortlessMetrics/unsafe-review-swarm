# unsafe-review

Static unsafe exposure analysis for Rust PRs.

Read-only editor diagnostics and hovers from unsafe-review ReviewCards.
Advisory evidence only: not memory-safety proof, not UB-free status,
not Miri-clean status, no witness execution, no source edits.

## Install

1. Build a VSIX from this folder.
2. Install with `code --install-extension <path-to-vsix>`.

## Support

See [SUPPORT.md](./SUPPORT.md).

> Note: This lane intentionally omits the extension icon binary because this review surface does not support binary files; publication lanes should add a compliant PNG icon before store submission.
