# Build cache setup

This document records the workbench build-cache relocation policy so a fresh
agent or machine can discover it without reading gitignored settings files.

## Problem

Rust builds write multi-GB `target/` directories and cargo caches to the
default locations (`./target/` in the repo root and `~/.cargo/`). On a
constrained drive (the drive holding the repo + worktrees), this can saturate
disk and cause git operations to fail mid-build.

## Policy

Relocate heavy caches to a high-capacity drive before starting agent builder
work. The cleanup/cache handoff is documented in
`docs/contributing/AGENT-ORCHESTRATION.md` §9 ("Reconcile and clean"), which
links back to this setup guide.

## Setup

Set these environment variables (or in `.claude/settings.local.json`, which is
gitignored — do not commit machine-specific paths):

```bash
# Point cargo target dir at a high-capacity drive
export CARGO_TARGET_DIR=/path/to/high-capacity-drive/rust-target

# Point cargo home (registry + cache) off the constrained drive
export CARGO_HOME=/path/to/high-capacity-drive/cargo
```

The paths above are illustrative — use whatever high-capacity drive your
machine has. The key invariant is: **the repo checkout and git operations
must not share a drive with multi-GB build output**.

## Verification

After setting up, verify the relocation:

```bash
echo "CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-not set (default ./target)}"
echo "CARGO_HOME=${CARGO_HOME:-not set (default ~/.cargo)}"
df -h .  # check the repo drive has headroom
du -sh "${CARGO_TARGET_DIR:-target}"  # confirm build output is on the relocated drive
```

## Agent builder convention (#1607 item 2)

Agents (Codex, Jules, Claude, etc.) that create worktrees and build locally
must record their `CARGO_TARGET_DIR` choice so a follow-up agent can discover
it without guessing. The convention:

1. Set `CARGO_TARGET_DIR` in the worktree's `.claude/settings.local.json`
   (gitignored — do not commit machine-specific paths).
2. Reference the relocation in the PR's `## Builder cleanup` section
   (`.github/PULL_REQUEST_TEMPLATE.md`) — check the "if `CARGO_TARGET_DIR` /
   `CARGO_HOME` were relocated" box and record the path + reason.
3. Run `cargo run --locked -p xtask -- cleanup-audit` before opening the PR
   to confirm no stray target dirs are left on the repo drive.

This makes the build-cache policy durable across agent handoffs: a fresh
agent reads the PR's Builder cleanup section, sees the `CARGO_TARGET_DIR`
note, and knows to relocate before building instead of silently switching
the target dir and leaving an unclearable dir on the repo drive.

## Worktree cleanup

After agent-builder PRs merge, remove the temporary worktree and verify no
stray target outputs were left on the repo drive:

```bash
git worktree remove <worktree-path> --force  # only after merge + clean status
git worktree prune
du -sh target/  # should be minimal or absent if CARGO_TARGET_DIR is relocated
```

## Trust boundary

Operational/workbench hardening only. No product behavior change; advisory
trust boundary intact. This document does not commit machine-specific paths
or gitignored settings.
