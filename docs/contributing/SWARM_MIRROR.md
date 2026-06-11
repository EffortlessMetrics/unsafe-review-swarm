# Swarm mirror-back (source → swarm)

After a release publishes from `unsafe-review` (source-of-record), the release
metadata must be mirrored back into `unsafe-review-swarm` (the workbench) and the
source-sync checkpoint advanced. This runbook is the source → swarm direction.

Do not confuse it with `SOURCE_HISTORY_CATCHUP.md`, which is the **opposite**
direction (swarm → source, a history-preserving merge commit that must not be
squashed). The two directions have different merge models on purpose.

## Direction and merge model

- swarm → source (release import): history-preserving **merge commit**, never
  squashed — see `SOURCE_HISTORY_CATCHUP.md`.
- source → swarm (this runbook): a normal **squash PR** to swarm `main`.

Squashing is correct here because swarm tracks absorption of source state through
`policy/source-sync.toml` (`acknowledged_source_main`), **not** through git
ancestry. The `source-divergence` / `check-source-sync` gates compare the
recorded checkpoint against source `main`; they do not require swarm history to
descend from source history.

## When to use

Use this runbook after a release has been published from source — crates are on
crates.io, the tag exists, and the GitHub Release is cut — and
`cargo run --locked -p xtask -- source-divergence` reports `source is ahead of
swarm`.

## What the mirror carries

1. Synchronized crate version bumps (`crates/*/Cargo.toml`) and `Cargo.lock`.
2. `docs/releases/<version>-*.md` (the release notes).
3. `docs/handoffs/<date>-release-<version>-preparation.md` (the source-side prep
   handoff).
4. The dated `## <version> - <date>` `CHANGELOG.md` section (see the convention
   below — this is the one part that is **not** a wholesale file copy).
5. New `docs/handoffs/<date>-source-<version>-publication-sync.md` (this mirror's
   own closeout) and its `docs/handoffs/README.md` index rows.
6. The advanced `policy/source-sync.toml` checkpoint.

## Required steps

Fetch source and branch off swarm `main`:

```bash
rtk git fetch public
rtk git switch -c sync/source-<version>-publication origin/main
```

Bring the version, lock, and release-note files straight from source (these are
safe wholesale checkouts):

```bash
rtk proxy git checkout public/main -- \
  Cargo.lock \
  crates/unsafe-review/Cargo.toml \
  crates/unsafe-review-cli/Cargo.toml \
  crates/unsafe-review-core/Cargo.toml \
  docs/releases/<version>-<slug>.md \
  docs/handoffs/<date>-release-<version>-preparation.md
```

Apply the CHANGELOG convention by hand (next section). Then write the
publication-sync handoff, add its `docs/handoffs/README.md` rows, and advance the
checkpoint in `policy/source-sync.toml`:

```toml
acknowledged_source_main = "<source main SHA>"
acknowledged_by = "docs/handoffs/<date>-source-<version>-publication-sync.md"
reason = "..."  # source release PR, crates.io versions, tag, release date
```

## CHANGELOG convention (do NOT wholesale-copy)

Swarm keeps in-flight entries under a single `## Unreleased` heading; source
converts them to a dated `## <version> - <date>` section at release-prep time. A
`git checkout public/main -- CHANGELOG.md` is **wrong** — source and swarm
diverge structurally below the release line, and source may carry artifacts swarm
should not (e.g. a duplicated dated section).

The mirror operation is:

1. Rename swarm's `## Unreleased` heading to `## <version> - <date>`.
2. Add the source release intro paragraph under the new dated header.
3. Restore a fresh, empty `## Unreleased` heading on top.
4. Reconcile entries: any entry present in source's dated section but missing
   from swarm's former `Unreleased` must be added, so the swarm dated section
   reaches content parity with source.

Verify parity (should print nothing):

```bash
diff \
  <(awk '/^## <version>/{f=1} f&&/^## <prev-version>/{exit} f' CHANGELOG.md) \
  <(rtk proxy git show public/main:CHANGELOG.md | awk '/^## <version>/{f=1} f&&/^## <prev-version>/{exit} f')
```

If source carries a duplicated or malformed dated section, do **not** mirror the
defect; keep swarm's CHANGELOG clean and note the source-side issue in the
publication-sync handoff for separate source-side repair.

## Validation

```bash
rtk cargo run --locked -p xtask -- check-source-sync
rtk cargo run --locked -p xtask -- source-divergence   # expect new_source_commits=0
rtk cargo fmt --all --check
rtk cargo check --workspace --all-targets --locked
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --locked
rtk cargo run --locked -p xtask -- check-pr
rtk proxy git diff --check
```

`check-source-sync` and `source-divergence` must report
`new_source_commits=0` ("no source commits after the acknowledged swarm sync
point"). That is the signal the divergence is absorbed and routine development
can resume.

## PR

Open a squash PR to swarm `main`:

```text
sync: mirror source <version> publication into swarm workbench
```

The body should state the source release PR, the crates.io versions, the tag and
release date, the checkpoint SHA, and any observed source-side follow-ups that
were intentionally not mirrored.

## Boundary

This runbook mirrors release metadata and moves a bookkeeping checkpoint. It does
not publish crates, change behavior, run witnesses, post comments, edit
downstream source, or start policy gating. It makes no memory-safety proof,
UB-free, Miri-clean, site-execution, calibrated precision/recall, or
policy-readiness claims.
