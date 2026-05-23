# Marketplace publication checklist

This guide is the operator-facing companion to
[docs/editor/extension-mvp.md](extension-mvp.md) and
[UNSAFE-REVIEW-SPEC-0021](../specs/UNSAFE-REVIEW-SPEC-0021-vscode-openvsx-extension.md).
It documents what must be true before any VS Marketplace or Open VSX
publication of the `unsafe-review` editor extension, and how the manual
`workflow_dispatch`-only publish workflow is invoked.

This is a checklist, not a one-click button. Marketplace publication is a
deliberate, owner-driven step. Nothing in this repository publishes
automatically.

## Trust boundary

`unsafe-review` is advisory unsafe contract review. It does not prove
memory safety, does not claim UB-free status, does not run Miri by default,
and does not enforce blocking policy by default. The same boundary must
appear in the published extension's marketplace listing, README, hover
footer, and status bar text. Nothing on the marketplace can claim a
stronger guarantee than the saved bundle does.

## Pre-publication owner setup

These are one-time owner steps that live outside this repository:

### VS Marketplace

1. Create or verify a Visual Studio Marketplace publisher whose ID matches
   `effortlessmetrics` (the `publisher` field in
   `editors/vscode/package.json`).
2. Generate an Azure DevOps Personal Access Token with the
   **Marketplace: Manage** scope. Scope it tightly: only that scope, only
   the organization that owns the publisher.
3. Add the token to the `EffortlessMetrics/unsafe-review-swarm` repository
   secrets as `VSCE_PAT`.

### Open VSX

1. Create or verify an Eclipse Foundation account that will own the
   `effortlessmetrics` namespace.
2. Sign the Eclipse Foundation Open VSX Publisher Agreement.
3. Generate an Open VSX access token from `open-vsx.org/user-settings/tokens`.
4. Create the namespace via
   `npx ovsx create-namespace effortlessmetrics -p $OVSX_PAT` (one-time).
5. Add the token to the repository secrets as `OVSX_PAT`.

Do not commit these tokens to the repository. Do not place them in
`.env` files or anywhere `git status` can see them.

## Extension-side checklist

Before invoking the publish workflow, confirm:

- [ ] `editors/vscode/package.json` `version` matches the version you
      intend to ship.
- [ ] `editors/vscode/package.json` `publisher` is `effortlessmetrics`.
- [ ] `editors/vscode/package.json` `displayName`, `description`, and
      `categories` are accurate. The description must not claim safety,
      UB-free status, Miri-clean status, or site-execution proof.
- [ ] `editors/vscode/README.md` includes the trust-boundary block at the
      top and the explicit non-goals list.
- [ ] `editors/vscode/CHANGELOG.md` has a current entry summarizing the
      change.
- [ ] `editors/vscode/LICENSE` and `SUPPORT.md` are present.
- [ ] `editors/vscode/icon.png` is present, if you want a store icon.
      Open VSX rejects SVG icons; use a PNG. Until an icon ships, the
      extension publishes without one; the `editor-extension-packaging`
      workflow does not require an icon binary.
- [ ] `editor-extension-packaging` CI passed on the commit you intend to
      publish.

## VSIX rehearsal

Before publishing, attach a VSIX to a GitHub Release and install it locally:

```bash
gh release create v0.1.0-vscode \
  --draft \
  --title "unsafe-review VS Code extension 0.1.0 (rehearsal)" \
  --notes "Rehearsal VSIX from editor-extension-packaging CI." \
  unsafe-review-vscode-0.1.0.vsix
code --install-extension unsafe-review-vscode-0.1.0.vsix
```

Confirm:

- the extension activates on a workspace containing
  `target/unsafe-review/lsp.json`,
- diagnostics, hovers, and code actions appear as expected,
- nothing claims memory safety, UB-free status, Miri-clean status, or
  site-execution proof in the editor UI,
- no source edits, witness executions, or subprocess invocations happen.

## Manual publish workflow

Once the checklist passes, run the manual publish workflow:

```bash
# Workflow file: .github/workflows/editor-publish.yml
# Trigger:       workflow_dispatch only
# Inputs:
#   version                     (required, e.g. "0.1.0")
#   dry_run                     (default true)
#   publish_to_vs_marketplace   (default false)
#   publish_to_open_vsx         (default false)
```

The workflow must be dispatched from `main`. The default invocation is a dry
run: build, test, package, and report what would be published. Publishing only
happens when `dry_run=false`, at least one publish target boolean is `true`,
and the matching token secret is configured.

Example invocations:

```bash
# Dry run (default; no publish): just package and report
gh workflow run editor-publish.yml --ref main -f version=0.1.0

# VS Marketplace only
gh workflow run editor-publish.yml \
  --ref main \
  -f version=0.1.0 \
  -f dry_run=false \
  -f publish_to_vs_marketplace=true

# Open VSX only
gh workflow run editor-publish.yml \
  --ref main \
  -f version=0.1.0 \
  -f dry_run=false \
  -f publish_to_open_vsx=true
```

The workflow:

- runs only on `workflow_dispatch`; no PR or push trigger can invoke it,
- fails when dispatched from any ref other than `main`,
- never publishes if `dry_run=true` (the default),
- never treats `dry_run=false` with no selected publish target as a
  successful publish,
- requires `VSCE_PAT` to publish to VS Marketplace and `OVSX_PAT` for Open
  VSX; missing tokens fail the relevant step rather than silently skip,
- uploads the VSIX as a workflow artifact regardless of publish mode,
- never runs `unsafe-review`, never executes witnesses, never edits source,
  never posts PR comments.

## After publication

After a successful publication:

- Tag the commit, for example `git tag v0.1.0-vscode && git push --tags`.
- Update `editors/vscode/CHANGELOG.md` with the published version row.
- Move `marketplace-publish-workflow` work item in
  `.unsafe-review-spec/lanes/marketplace-first-hour-ux/tracker.toml` from
  `blocked` to `done`.
- Replace the README's `extension planned` badges with
  `installs` / `downloads` shields for the published listings.
- Add the marketplace and Open VSX URLs to
  `docs/deferred/editor-extension.md` (rename or supersede that doc as
  appropriate).

## What this workflow never does

- It never publishes on PR merge.
- It never publishes on push to main.
- It never publishes when `dry_run` is true.
- It never publishes without the matching publish boolean being true.
- It never claims memory safety, UB-free status, Miri-clean status, or
  site-execution proof on the marketplace listing.
- It never runs `unsafe-review`, Miri, sanitizers, Loom, Shuttle, Kani,
  Crux, or any other analyzer.
