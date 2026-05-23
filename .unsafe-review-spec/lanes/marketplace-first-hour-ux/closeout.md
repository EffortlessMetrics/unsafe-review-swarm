# Marketplace and first-hour UX lane closeout

Status: shipped (pending owner-side marketplace publication and
analyzer-side comment-plan-actionability work)

The lane took `unsafe-review` from "published on crates.io" to "a Rust
maintainer can install it, wire it into PRs, see value, and optionally use
it in an editor", without weakening the advisory ReviewCard trust boundary.
Status per work item below; PR receipts are listed where applicable.

## First-hour guide — done (#356)

- `docs/FIRST_HOUR.md` exists with one runnable install/run/explain/support
  path and explicit non-goals.
- `README.md` and `docs/FIRST_USE.md` link to it.

## First-hour smoke — done (#356)

- `cargo run --locked -p xtask -- check-first-hour` exists and validates
  the documented first-hour structure: required commands, required artifact
  paths, fixture reference, trust-boundary text, no overclaims, and the
  required inbound links from README and FIRST_USE.
- The command is documented in the xtask help text and the first-hour
  guide.

## GitHub Actions user guide — done (#357)

- `docs/ci/github-actions.md` exists with a single canonical example
  referenced by `.github/examples/unsafe-review-first-pr.yml`.
- Default behavior: read-only token, advisory packet, artifact upload,
  bounded job summary, no comments, no witnesses, no blocking.

## GitHub summary fragment — done (#363)

- `unsafe_review_core::render_github_summary` produces
  `target/unsafe-review/github-summary.md` as the seventh `first-pr`
  bundle artifact.
- `.github/workflows/unsafe-review.yml` and
  `.github/examples/unsafe-review-first-pr.yml` `cat` the fragment into
  `GITHUB_STEP_SUMMARY` instead of the previous awk slice over
  `pr-summary.md`.
- `check-first-pr-artifacts` requires the fragment, rejects re-inclusion of
  the card table or witness plan, and caps its word count.

## Comment-plan actionability — in_progress

- Continues under `plans/post-0.2.0/comment-plan-quality.md`.
- Documentation surface extended in #364 with copy-range and
  public-unsafe-fn-missing-safety fixture-backed examples.
- #368 added `relevance` metadata to selected and not-selected
  `comment-plan.json` entries and made `check-first-pr-artifacts` reject
  missing or unknown relevance values.
- Deeper analyzer-side selection and ranking work remains tracked
  separately.

## Extension MVP spec — done (#358)

- `docs/editor/extension-mvp.md` defines the saved-lsp viewer MVP scope and
  non-goals. The MVP path is explicitly frozen against live LSP wiring.

## Extension saved-lsp viewer — done (#359)

- `editors/vscode/` loads `lsp.json` and publishes diagnostics, hovers, and
  command-only actions: Refresh Bundle, Open PR Summary, Open Witness Plan,
  Copy Agent Packet Command, Copy Witness Command.
- No source edits, no witness execution, no PR comment posting, no live
  LSP server, no telemetry.
- Bundle parser has unit tests via `node --test`; the extension wakes only
  via `workspaceContains` / `onLanguage:rust` / explicit command palette
  activation.

## Extension packaging smoke — done (#360)

- `.github/workflows/editor-extension.yml` compiles, tests, and packages
  the VSIX on PR / push / dispatch (paths-scoped to `editors/vscode/**`
  plus relevant policy files). Uploads `unsafe-review-vscode.vsix` as a
  workflow artifact.
- `policy/workflow-allowlist.toml` and `policy/ci-lane-whitelist.toml`
  carry matching entries (`workflow-0004`, lane `editor-extension-packaging`).

## Marketplace publish workflow — done (#361)

- `.github/workflows/editor-publish.yml` is `workflow_dispatch`-only, must
  be dispatched from `main`, refuses to publish unless `dry_run=false`
  plus a publish target is set, validates package identity and version
  against `editors/vscode/package.json`, and requires `VSCE_PAT` /
  `OVSX_PAT` secrets only for the matching publication path.
- `docs/editor/marketplace-publication.md` documents the owner-side
  prerequisites (publisher accounts, namespace creation, secret
  configuration) and the example dispatch invocations.
- An actual marketplace publication still requires the owner to provision
  the accounts / namespace / secrets and to run the workflow manually.

## Codecov advisory — done (#362 + #365)

- `.github/workflows/coverage.yml` runs `cargo llvm-cov` and uploads LCOV
  via `codecov/codecov-action@v5` with `fail_ci_if_error: false`.
  `policy/workflow-allowlist.toml` (`workflow-0006`) and
  `policy/ci-lane-whitelist.toml` (lane `coverage`) carry matching
  advisory entries.
- `codecov.yml` makes Codecov's project / patch status checks
  informational so they never block PR merge.
- README carries an advisory `coverage (advisory)` badge that links to
  `docs/ci/coverage.md`, where the not-memory-safety / not-UB-free /
  not-Miri-clean / not-site-execution / not-unsafe-correctness boundary
  is restated.

## Comment-plan examples — done (#364)

- `docs/ci/COMMENT_PLAN_EXAMPLES.md` covers selected (raw_pointer_alignment,
  copy_nonoverlapping, public_unsafe_fn_missing_safety),
  not-selected (ffi_sanitizer_route), and no-card (safe_code_no_cards)
  shapes. Each example is fixture-backed and verified with
  `check-first-pr-artifacts`.

## Out-of-scope follow-ups

- Actual VS Marketplace / Open VSX publication: owner-side
  `VSCE_PAT` / `OVSX_PAT` provisioning, publisher account, Open VSX
  namespace, manual `gh workflow run editor-publish.yml --ref main`.
- Editor extension store icon (`editors/vscode/icon.png`): optional, lands
  as a separate small follow-up once an icon binary exists.
- `comment-plan-actionability` analyzer work: continues under
  `plans/post-0.2.0/comment-plan-quality.md`.
