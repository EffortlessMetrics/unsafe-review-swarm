# Workbench Cleanup Audit — 2026-06-13

Scope: swarm primary checkout `H:/Code/Rust/unsafe-review-swarm`, validated against live git/gh state for the 0.3.7 release-readiness package. No builds run. All removal commands marked **[OWNER]** are irreversible (destroy local-only state) and must be confirmed by the owner before execution.

## Git Worktrees (registered)

**Primary Checkout**
- Path: `H:/Code/Rust/unsafe-review-swarm`
- Branch: `main` (`ba3440ed`, 2026-06-13)
- Status: Clean (`git status --short` empty)
- Classification: OWNER-OWNED (primary checkout) — leave untouched.

**Worktree: release-gate**
- Path: `H:/Code/Rust/unsafe-review-swarm/.claude/worktrees/release-gate`
- Branch: `build/base` (`1b1cf88c`)
- Last commit: 2026-06-07 04:58:45 -0400 — `ci: bump ub-review to structured-ingestion SHA (#363 + #365) (#1548)`
- Status: Clean
- Merge status: `build/base` IS an ancestor of `main` — landed via PR #1548 (MERGED 2026-06-07T08:58:46Z).
- Classification: VERIFY-FIRST — branch is merged, so the worktree is stale; retain only if release-gate re-runs need it. Not dirty, not blocking. **[OWNER]** before removal.

**Worktree: wt-ubreview-ctx**
- Path: `H:/Code/Rust/unsafe-review-swarm/.claude/worktrees/wt-ubreview-ctx`
- Branch: `ci/feed-unsafe-review-to-ubreview` (`bcb9988f`)
- Last commit: 2026-06-07 03:26:15 -0400 — `ci: fold advisory unsafe-review coverage snapshot into ub-review precontext`
- Status: Clean
- Merge status: NOT an ancestor of `main` — PR #1541 was CLOSED (not merged) 2026-06-07T08:31:04Z.
- Classification: VERIFY-FIRST — unmerged work behind a closed PR. Do NOT delete without owner confirmation; may be intentionally saved experimental work. **[OWNER]** before removal.

## Orphaned Local Branches (no registered worktree)

Verified counts from `git branch -a`:

- **23** `worktree-agent-*` branches
- **2** `worktree-wf_8fac77e2-e4b-*` branches
- **1** `worktree-release-gate` branch (`e8cfcec5`, `analysis: flag stale JS-buffer span use after reentry (#1508)`) — distinct from `build/base`; merged into `main`.

Total orphaned `worktree-*` branches: **26**.

Ancestry resolved against `main` (`git merge-base --is-ancestor`):

- **21 of the 25 agent/wf branches are MERGED** → safe to delete with `git branch -d <branch>` (git refuses if not actually merged).
- **`worktree-release-gate` is MERGED** → safe with `git branch -d worktree-release-gate`.
- **4 agent branches are UNMERGED** with recent (2026-06-12) in-flight work — DO NOT delete; treat as VERIFY-FIRST / owner-decision:
  - `worktree-agent-a1462c5a8bb96f38e` — `cli: add pr alias with auto-detect root/base for zero-config entry point`
  - `worktree-agent-a55beb3440181c267` — `review: reconcile docs with forbid->deny + link ADR-0008 to #1620`
  - `worktree-agent-a60e420ee27d4c19d` — `ci: add smoke-action workflow to close SPEC-0037 acceptance gap`
  - `worktree-agent-a7e04199f8420cd74` — `corpus: build binary once before fixture loop so elapsed_ms reflects scan not compile`

Bulk-delete the merged set (non-destructive — `-d` only deletes merged branches):

```bash
git branch --merged main | grep -E 'worktree-(agent-|wf_|release-gate)' | xargs -r -n1 git branch -d
```

Force-deleting the 4 unmerged branches (`git branch -D`) is **irreversible and [OWNER]-gated** — they carry unmerged commits not reachable from any other ref.

## Untracked / Uncommitted Files

Primary checkout: clean — `git status --short` returns no output. No stray `*_out.json`, `*.patch`, or `*.diff` in repo root. Classification: CLEAN.

## Generated Artifacts in `target/` (gitignored via `/target/`)

### `target/unsafe-review/` — 136 KB
- Analyzer output bundle: `cards.json`, `cards.sarif`, `pr-summary.md`, `github-summary.md`, `lsp.json`, `policy-report.{json,md}`, `receipt-audit.{json,md}`, `repair-queue.json`, `witness-plan.md`, `usefulness-telemetry.json`, and related projections.
- Classification: SAFE-TO-REMOVE — regenerated on next run; no retention value.

### `target/dogfood-work/` — 102 MB
- 7 full crate clones: `arrayvec`, `bytes`, `crossbeam`, `hashbrown`, `memchr`, `mio`, `smallvec`.
- **14** result JSONs (each repo has both a `*.uncapped.json` and a `*.unsafe-review.after*/capped.json`) plus **14** `*.status.json` artifacts.
- Classification: VERIFY-FIRST — working copies from dogfood/corpus validation (SPEC-0038 usefulness telemetry, SPEC-0039 corpus backstop). Keep if dogfood re-runs are planned (avoids network re-fetch; 102 MB is modest); remove if validation is complete. **[OWNER]** before removal.

### `target/debug/` — **1.8 GB** (measured)
- Compiled binaries plus PDB symbols (`xtask`, `unsafe-review`, `cargo-unsafe-review`); `.pdb` files dominate.
- Classification: SAFE-TO-REMOVE — regenerated on next `cargo build`; largest single reclaim. `target/` must not be edited by concurrent processes, but a full debug-dir delete between sessions is low-risk.

### `target/tmp/` — empty
- Classification: SAFE-TO-REMOVE (no-op; currently empty).

## Background Processes

No active `cargo`/`rustc`/`cmake`/`make`/watcher processes. Only `rtk` (token proxy) running as expected. Classification: CLEAN.

## Summary Table

| Candidate | Class | Evidence | Action |
|-----------|-------|----------|--------|
| `.claude/worktrees/release-gate` (`build/base`) | verify-first | `1b1cf88c` merged via #1548 (2026-06-07); clean | **[OWNER]** confirm before `git worktree remove` |
| `.claude/worktrees/wt-ubreview-ctx` (`ci/feed-unsafe-review-to-ubreview`) | verify-first | PR #1541 CLOSED, not merged; branch unmerged; clean | **[OWNER]** confirm before remove |
| 21 merged `worktree-agent/wf` + `worktree-release-gate` | safe-to-remove | ancestors of `main` | `git branch -d` (non-destructive) |
| 4 unmerged `worktree-agent-*` (a1462c5a, a55beb34, a60e420e, a7e04199) | verify-first | unmerged, in-flight work dated 2026-06-12 | **[OWNER]** — do NOT `git branch -D` without confirmation |
| `target/unsafe-review/` (136 KB) | safe-to-remove | regenerated build output | `rm -rf .../target/unsafe-review` |
| `target/dogfood-work/` (102 MB) | verify-first | SPEC-0038/0039 corpus working copies | **[OWNER]** `rm -rf .../target/dogfood-work` only if re-runs not planned |
| `target/debug/` (1.8 GB) | safe-to-remove | binaries + PDB symbols; regenerated | `rm -rf .../target/debug` |
| `target/tmp/` (empty) | safe-to-remove | scratch, empty | `rm -rf .../target/tmp` |
| Primary checkout (`main`) | owner-owned | clean; `ba3440ed`; active | leave as-is |
| Background processes | none | only rtk running | none |

## Counts by Classification

- **safe-to-remove:** `target/unsafe-review` (136 KB) + `target/debug` (1.8 GB) + `target/tmp` (empty) + 22 merged branches → ~1.8 GB disk recoverable.
- **verify-first:** `target/dogfood-work` (102 MB); 2 registered worktrees; 4 unmerged agent branches → confirm owner intent.
- **owner-owned:** 1 item (primary checkout).
- **clean/none:** uncommitted files, stray patches, background jobs.

## Hard Boundaries — Do NOT Remove Without [OWNER]

1. Primary checkout (`H:/Code/Rust/unsafe-review-swarm`) — active, clean.
2. `ci/feed-unsafe-review-to-ubreview` (wt-ubreview-ctx) — unmerged behind closed PR #1541; possible saved experiment.
3. The 4 unmerged `worktree-agent-*` branches (2026-06-12 in-flight: pr alias, smoke-action/SPEC-0037, corpus timing, docs reconcile) — force-deletion is irreversible.
4. `target/dogfood-work` — owner decides; deletion forces network re-fetch on next dogfood run.

## Recommended order of operations

1. Safe reclaim first (non-destructive / regenerable): delete `target/debug`, `target/unsafe-review`, `target/tmp`; run the `git branch --merged` bulk-prune (safe by construction).
2. **[OWNER]** decisions next: dogfood-work retention; the two registered worktrees (`git worktree remove`); the 4 unmerged agent branches.
3. Irreversible steps (`git branch -D`, `git worktree remove` of unmerged work, `rm -rf target/dogfood-work`) only after explicit owner go-ahead.

## Claim-boundary scan

No forbidden trust-boundary claims present. This is a workbench/disk-hygiene audit; it makes no memory-safety, UB-detection, Miri-clean, site-execution, or precision/recall claims, and proposes no default comment-posting, blocking, witness execution, or silent source edits. CLEAN.
