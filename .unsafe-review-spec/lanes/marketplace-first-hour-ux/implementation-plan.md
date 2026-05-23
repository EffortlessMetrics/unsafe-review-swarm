# Marketplace and first-hour UX lane implementation plan

## Goal

Turn `unsafe-review` from a published crate into a tool a Rust maintainer can
actually adopt:

```text
cargo install unsafe-review --locked
unsafe-review first-pr --base origin/main
```

Then, for editor users:

```text
install unsafe-review extension from VS Marketplace / Open VSX
load target/unsafe-review/lsp.json
see diagnostics, hovers, copy agent packet, copy witness command
```

All of it must preserve the advisory ReviewCard trust boundary.

## Sequencing

1. Add `docs/FIRST_HOUR.md` as the maintainer first-hour walkthrough and link
   it from README and `docs/FIRST_USE.md`.
2. Add `cargo run --locked -p xtask -- check-first-hour` to keep the
   documented first hour runnable: doctor, fixture first-pr, artifact verify,
   no-card fixture path.
3. Add `docs/ci/github-actions.md` as the copy-paste user-facing GitHub Actions
   guide and re-anchor `.github/examples/unsafe-review-first-pr.yml` as the
   canonical drop-in.
4. Add a bounded `github-summary.md` job-summary fragment so CI does not have
   to dump the full PR summary into `GITHUB_STEP_SUMMARY`.
5. Continue the `plans/post-0.2.0/comment-plan-quality.md` work for
   reviewer-actionability metadata, ranking, and not-selected reasons. No
   posting.
6. Define the editor extension MVP as a saved-`lsp.json` viewer in
   `docs/editor/extension-mvp.md`, frozen against any live LSP wiring.
7. Implement the MVP saved-LSP viewer in `editors/vscode/` with command-only
   actions for Copy Agent Packet, Copy Witness Command, Open PR Summary, and
   Refresh. No source edits, no witness execution.
8. Add extension packaging smoke that produces a VSIX and attach the VSIX to a
   GitHub Release before any marketplace publication.
9. Add a manual `workflow_dispatch`-only extension publish workflow guarded by
   `dry_run`, explicit publish booleans, and dedicated secrets.
10. After the first successful Codecov upload, add an advisory coverage badge
    documented as execution-surface telemetry, not unsafe correctness evidence.

## Non-goals

- Automatic PR comment posting by default.
- Default blocking on unsafe-review findings.
- Source-editing quick fixes.
- Default witness execution (Miri, cargo-careful, Loom, Kani, sanitizers).
- Live LSP wiring in the MVP extension (deferred to the SPEC-0018 hardening
  gate).
- Automatic crate or extension publication on PR merge.
- Safety, UB-free, Miri-clean, site-execution, or calibrated precision claims.

## Exit criteria

- A maintainer can install from crates.io, run `first-pr` against their branch,
  open `target/unsafe-review/pr-summary.md`, and run `explain` on the top card
  using only documented commands from `docs/FIRST_HOUR.md`.
- A GitHub Actions user can paste `.github/examples/unsafe-review-first-pr.yml`
  into their repository and get an advisory PR job that uploads artifacts and
  writes a bounded job summary.
- The editor MVP extension can load a saved `lsp.json`, publish read-only
  diagnostics and hovers, and copy bounded agent packets and witness commands
  without editing source or running witnesses.
- README marketplace badges read `installs` and `downloads` instead of
  `planned` only after real Marketplace and Open VSX publications exist.

## Claim boundary

This lane improves install, CI, and editor reach. It does not promote any
unsafe-review surface to a blocking gate, post comments by default, edit
source, execute witnesses, or claim safety / UB-free / Miri-clean status.
