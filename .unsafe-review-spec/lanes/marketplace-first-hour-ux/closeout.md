# Marketplace and first-hour UX lane closeout

Status: open

Closeout fills in as work items in `tracker.toml` move from `proposed` /
`ready` / `in_progress` to `done`. Until then, this file documents the
intended evidence shape for each work item.

## First-hour guide

- `docs/FIRST_HOUR.md` exists with one runnable install/run/explain/support
  path and explicit non-goals.
- `README.md` and `docs/FIRST_USE.md` link to it.

## First-hour smoke

- `cargo run --locked -p xtask -- check-first-hour` exists and exercises:
  - `unsafe-review doctor`,
  - `unsafe-review first-pr` against a known fixture,
  - `check-first-pr-artifacts` on the output directory,
  - a no-card fixture path,
  - `unsafe-review support`.
- The command is documented in the xtask help text and the first-hour guide.

## GitHub Actions user guide

- `docs/ci/github-actions.md` exists with a single canonical example referenced
  by `.github/examples/unsafe-review-first-pr.yml`.
- Default behavior: read-only token, advisory packet, artifact upload, bounded
  job summary, no comments, no witnesses, no blocking.

## GitHub summary fragment

- `target/unsafe-review/github-summary.md` is produced by the first-pr command
  alongside the existing bundle.
- `check-first-pr-artifacts` validates the fragment shape.

## Comment-plan actionability

- `comment-plan.json` exposes `selection_reason`, `actionability`, `relevance`,
  and `not_selected[]` entries with documented reasons. Body length and
  duplicate rules enforced in the artifact verifier.
- No workflow posts comments by default.

## Extension MVP spec

- `docs/editor/extension-mvp.md` defines the saved-lsp viewer MVP scope and
  non-goals. The MVP path is explicitly frozen against live LSP wiring.

## Extension saved-lsp viewer

- `editors/vscode/` loads `lsp.json` and publishes diagnostics, hovers, and
  command-only actions: Copy Agent Packet, Copy Witness Command, Open PR
  Summary, Refresh.
- No source edits, no witness execution, no PR comment posting.

## Extension packaging smoke

- `npx @vscode/vsce package` produces a VSIX; CI verifies package contents.
- The first publishable VSIX is attached to a GitHub Release, not pushed to
  Marketplace or Open VSX.

## Marketplace publish workflow

- A `workflow_dispatch`-only publish workflow exists with `version`,
  `dry_run`, `publish_to_vs_marketplace`, and `publish_to_open_vsx` inputs.
- Secrets are scoped: `VSCE_PAT` and `OVSX_PAT`. No PR-triggered publication.

## Codecov advisory

- A coverage workflow uploads LCOV from `cargo llvm-cov` as advisory
  execution-surface telemetry. README badge is added only after the first
  successful upload and is captioned as telemetry, not unsafe correctness.
