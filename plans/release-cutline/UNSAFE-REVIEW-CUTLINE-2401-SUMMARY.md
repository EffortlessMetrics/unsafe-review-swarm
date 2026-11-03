# 0.6 delivery inventory and proposed cutline — human summary (issue #2401, PR1)

Machine-readable source:
[`UNSAFE-REVIEW-CUTLINE-2401.toml`](UNSAFE-REVIEW-CUTLINE-2401.toml).
Status: **draft inventory** — it freezes nothing. A follow-up PR refreshes the
SHAs and flips the status after #2400 and the chosen fixes/dependencies settle.

Audited 2026-11-02 against swarm main
`9a0f5bf4b8a752794ff75eb738c29edaeb71a843` and source main
`735fb7ba1107b79a9c18bd9b6bf169eeeae68cd1`
(`source-divergence`: `new_source_commits=0`).

## What 0.6 would ship

- **Baseline:** the published 0.5.0 set (crates `0.5.0` on the index,
  `--short` output, `SAFETY:` evidence, caller-contract routing,
  `get_unchecked` fold, witness test-first cue, slot counts, 0/1/2 exits).
- **Integrated workbench deltas:** verdict-aware adverse cards, reach
  refinements, non-null-vs-liveness evidence split, new detector families
  (static-mut, union field reads, target-feature guards), receipt subject
  gating, outcome unknown bucket, movement evaluation, and the merged asm
  contradiction behavior (name it + route to the code fix, #2356 + #2406).
- **Open decisions:** Windows baseline consolidation #2394
  (optional-before-freeze; six constituents #2381/#2384/#2386/#2388/#2390/#2392
  counted once under it), parser bump #2349 (deferred pending focused
  deltas), dependency group #2404 (deferred pending per-package freeze
  examination — rationale per package is in the TOML), central CI migration
  #2380 (deferred to its own lane). Source mirrors #575/#582 stay owner-gated
  on the single promotion path. #2397 is closed unmerged, superseded by #2404.
  #2356 is merged and is never listed as open. The #2301 typed-evidence model
  remainder is deferred out of 0.6 scope.

## Key facts that must not be misread

- Tag `v0.5.0` targets source `f1eda818`; GitHub Release `v0.5.0` published
  `2026-09-17T05:57:40Z`. The receipt left both to a separate go; the handoff
  records both as done. Target and flag stay distinct.
- Workbench manifests read `0.3.8` while the registry holds `0.5.0`.
  `0.6.0` is proposed-only; the bump lands with release-prep on its side
  branch when the API/CLI/schema/MSRV assessment settles it.
- Compatibility: no schema-version bump, no removal — four additive
  post-0.5.0 CLI spellings (scope, environment, work, agent) with
  defaults/aliases/0-1-2 exits unchanged — plus only stricter, more
  explicit states (unknown bucket, subject gating). Legacy rejected/unknown/stale artifacts stay explicit; they never
  become successful empty reports. Consumer qualification (#1921-style matrix
  against tokmd `1.15.0`, ub-review, saved-editor) is still open.
- MSRV stays Rust `1.98` / qualification `1.98.1`; no platform promise is added
  (Windows qualification needs #2394; the draft claims no native qualification).
- Lockfile SHA-256 at the candidate: `c3ad9ce9…51a0164` (method pinned in
  the TOML); `ra_ap_syntax 0.0.350`, `sha2 0.11.0`, `serde_json 1.0.151`,
  `tokio 1.53.1`, `signal-hook 0.4.4`, `toml 1.1.5+spec-1.1.0`,
  `proptest 1.11.0`.

## Rules

Freeze amendments name the defect, changed bytes/contracts, and affected
proof; rerun invalidated evidence; never rebind old runs by editing dates.
0.4.0/0.5.0 history is immutable. Stale in-place panels are evidence only
(#2398 owns them). No publication, tag, merge, or qualification follows from
this paper slice.

Claim boundary: `unsafe-review` finds unsafe Rust changes missing a safety
contract, guard, test, or witness. Nothing here proves safety, UB-freedom,
Miri-cleanliness, site execution, precision/recall, or release qualification.
