# 2026-06-01 - closed PR recovery audit

Scope: restore truthful disposition after a release-cleanup pass closed useful or
potentially useful PRs. The correction rule is:

```text
Restore visibility first. Judge second.
```

This is not a release, not a queue-cleanup success claim, and not a claim that
the reopened PRs are mergeable. It records the affected PRs, current visibility,
and the evidence required before any future close can stand.

## Required disposition states

Every affected PR must end in exactly one of these states:

- open and awaiting review, fix, rebase, CI, owner decision, or a later lane;
- merged after review and validation;
- closed as duplicate or superseded by a named merged PR or commit;
- recreated from current `main` with the original PR linked;
- explicitly abandoned by owner decision.

Forbidden closure reasons include release timing, queue cleanup, clean queue
optics, review quota, CI budget, stale local checkout state, and "probably
stale" without current-main inspection.

## Current audit table

| PR | Title | Head branch | Branch exists | Current state | Files | Disposition | Replacement | Evidence / next step |
|---:|---|---|---|---|---|---|---|---|
| #1212 | ci: add MiniMax Droid PR review bot | `codex/setup-minimax-m3-for-droid-pr-review-bot` | yes | open, mergeable | workflow/spec/policy | fix before merge | n/a | Droid found P1/P2 issues: use `actions/checkout@v6`, fix MiniMax API key expansion, set `MINIMAX_API_KEY` env in config step, remove `id-token: write`, `issues: write`, and unneeded `actions: read`, add `timeout-minutes`, and align workflow allowlist permissions. |
| #1168 | sync: mirror source 0.3.2 preparation | `sync/source-main-6d660916` | no remote ref | closed | versions/release docs/source-sync | closed needs explicit replacement proof | claimed #1167 | Reopen failed because the head ref is gone. Existing close comment names #1167 and source main `6d660916`; verify #1167 fully mirrors the preparation state before treating this as final. |
| #1179 | Refactor analysis pipeline into focused submodules | `codex/refactor-complex-module-into-dry-srp-submodules` | yes | open, conflicting | pipeline/card builder/progress/source/summary/tests | pending recovery audit | none final | Pipeline is high risk because current main has `RepoScanEvent` and partial `AnalyzeOutput` semantics. Keep open until compared against current pipeline; recreate from current main if useful. |
| #1181 | Refactor agent output into focused modules | `codex/refactor-complex-module-into-submodules` | yes | open, conflicting | agent/context/packet/queue/tests | pending recovery audit | claimed #1180 | Compare with merged #1180 before closing. Preserve any distinct context/packet extraction or tests by recreating from current main. |
| #1182 | Refactor scanner file pass into SRP submodule | `codex/break-down-complex-function-into-modules` | yes | open, conflicting | scanner/file_scan | pending recovery audit | claimed #1190 | Compare with merged #1190 before closing. If only duplicate file-level scan extraction remains, close with #1190 named; otherwise recreate current-main delta. |
| #1185 | Refactor agent repairs into focused modules | `codex/refactor-complex-function-into-dry-srp-modules` | yes | open, conflicting | agent/repairs boundary/pointer/value | pending recovery audit | claimed #1183 | Compare with merged #1183. Preserve distinct family split if still useful; otherwise close only with #1183 named. |
| #1186 | Refactor LSP projection actions and hovers into submodules | `codex/break-complex-function-into-dry-srp-submodules-xq1oiu` | yes | open, conflicting | LSP projection/code_actions/hover | pending recovery audit | claimed #1184 | Compare with merged #1184. Close only if it is the same LSP helper split. |
| #1188 | Refactor obligation mappings into focused modules | `codex/refactor-complex-function-into-dry-srp-modules-h8i6mw` | yes | open, conflicting | obligations hazards/safety and xtask constants | pending recovery audit | claimed #1187 | Compare with merged #1187. Macro/xtask parser differences must be judged explicitly, not discarded as queue cleanup. |
| #1189 | Refactor advisory artifact set checker into artifact_set submodule | `codex/break-complex-function-into-dry-srp-submodules-czz4uv` | yes | open, conflicting | xtask advisory artifacts | pending recovery audit | claimed #1204 | Existing comment says #1204 was the smaller replacement. Verify exact overlap before closing. |
| #1191 | Refactor analysis pipeline into focused modules | `codex/break-down-complex-function-into-modules-swigmz` | yes | open, conflicting | pipeline discovery/progress/scan | pending recovery audit | none final | Pipeline split predates current partial-output flow. Keep open until compared against current `RepoScanEvent` and partial snapshot semantics; recreate if useful. |
| #1192 | Refactor agent packet projection into focused submodules | `codex/refactor-complex-module-into-submodules-i9bfqd` | yes | open, conflicting | agent context/evidence/tests/witness | pending recovery audit | claimed #1180/#1183 | Compare with current `agent/model.rs`. If context/evidence/witness split remains useful, recreate from current main. |
| #1193 | refactor: split agent output projection into focused modules | `codex/refactor-complex-module-into-dry-srp-submodules-0l96wn` | yes | open, conflicting | agent context/evidence/model/queue/readiness/repairs | pending recovery audit | claimed #1180/#1183 | Compare with current agent model/queue/repair layout. Close only if duplicate; otherwise recreate narrower remaining split. |
| #1194 | refactor agent output into submodules | `codex/refactor-complex-module-into-dry-srp-submodules-qm4bdc` | yes | open, conflicting | agent context/evidence/packet/queue/repairs | pending recovery audit | claimed #1180/#1183 | Same agent cluster; preserve any distinct useful model/packet split from current main. |
| #1198 | Split obligations catalog into focused submodules | `codex/locate-complex-function-and-refactor-d7wsud` | yes | open, conflicting | obligations catalog/pointer/memory/value/boundary | pending recovery audit | claimed #1187 | Deeper catalog split may be useful after #1187. Must compare to current obligations layout before closure. |
| #1200 | Refactor pipeline helpers into focused modules | `codex/refactor-complex-module-into-submodules-ak0vdb` | yes | open, conflicting | pipeline action_summary/card_identity/input_loading/progress/summary | pending recovery audit | none final | Pipeline helper extraction must preserve current status, event, interruption, and partial artifact semantics. Keep open pending current-main redesign decision. |
| #1201 | Refactor agent output into focused submodules | `codex/refactor-complex-module-into-dry-srp-submodules-n1qeha` | yes | open, conflicting | agent model/packet/queue/readiness/repairs/tests | pending recovery audit | claimed #1180/#1183 | Agent cluster alternate. Compare before closure; recreate distinct current-main delta if useful. |
| #1202 | Refactor agent packet output into focused submodules | `codex/refactor-complex-module-into-dry-srp-submodules-yksxdy` | yes | open, conflicting | agent context/evidence/queue/readiness/repairs/tests | pending recovery audit | claimed #1180/#1183 | Agent cluster alternate. Do not close until current-main overlap is explicit. |
| #1203 | Refactor scanner: split `scan_file` into focused modules | `codex/break-complex-function-into-dry-srp-submodules-wkuiqk` | yes | open, conflicting | scanner change_filter/fallback/syntax_projection | pending recovery audit | claimed #1190/#1195/#1196 | Compare against current scanner `file_scan`, `scan_site`, and `js_buffer_reentry` layout. Recreate useful fallback/syntax split if not covered. |
| #1205 | Refactor agent repair builder into SRP submodules | `codex/break-complex-function-into-dry-srp-submodules-n27uj4` | yes | open, conflicting | agent repairs followup/operation/repair_list | pending recovery audit | claimed #1183 | Compare against current repair modules. Preserve distinct repair-list/followup extraction if useful. |
| #1206 | refactor(scanner): split site collection into fallback and syntax submodules | `codex/locate-complex-function-and-refactor-nljk6q` | yes | open, conflicting | scanner fallback_scan/syntax_scan | pending recovery audit | claimed #1190/#1195/#1196 | Scanner alternate. Must compare against current scanner layout before closure. |
| #1207 | Refactor LSP projection helpers into `actions` and `hover` submodules | `codex/break-down-complex-function-into-modules-xiypzh` | yes | open, conflicting | LSP projection actions/hover | pending recovery audit | claimed #1184 | LSP alternate. Close only if duplicate of #1184 after comparison. |
| #1208 | Refactor agent output into focused submodules | `codex/refactor-complex-module-into-submodules-mnx2q6` | yes | open, conflicting | agent model/queue/tests | pending recovery audit | claimed #1180 | Agent cluster alternate. Compare with #1180 and current model/queue layout before closure. |
| #1210 | Refactor agent packet projections into SRP submodules | `codex/refactor-complex-module-into-dry-srp-submodules-z025py` | yes | open, conflicting | agent model/queue/readiness | pending recovery audit | claimed #1180 | Agent cluster alternate. Keep open pending explicit duplicate/current-main decision. |
| #1214 | sync: mirror source badge plus count | `sync/source-badge-plus-count` | no remote ref | closed | badge payload/docs/source-sync | closed needs explicit replacement proof | claimed `49c247f1`/#1213 | Reopen failed because the head ref is gone. Existing comment says main already mirrors source badge evidence-quality count; verify merged #1213/commit before treating final. |
| #1228 | sync: mirror source main 8427bf31 | `sync/source-main-8427bf31` | yes | open, parked | source-sync branch currently shows no file delta in PR metadata | do not merge as-is | redo sync if needed | Current comment says branch preserves source ancestry but is stale relative to swarm main and would delete recent swarm-only fixtures/calibration rows. Keep open/parked until sync need is resolved from current main. |
| #1231 | sync: acknowledge source main 8427bf31 | `sync/source-main-8427bf31-ledger` | no remote ref | closed | source-sync handoff and policy | closed needs explicit replacement proof | claimed `a8759fdf` | Reopen failed because the head ref is gone. Existing comment says `a8759fdf` already acknowledges source main `8427bf31`; verify before treating final. |

## Module-family recovery order

1. `#1212` first: fix Droid findings, validate `check-ci-lanes`,
   `check-docs`, `check-pr`, `source-divergence`, and `git diff --check`, then
   merge if green.
2. Sync/accounting PRs: `#1168`, `#1214`, `#1228`, `#1231`. Close or recreate
   only with exact replacement evidence.
3. Agent/output cluster: `#1210`, `#1208`, `#1205`, `#1202`, `#1201`, `#1194`,
   `#1193`, `#1192`, `#1185`, `#1181`.
4. Scanner cluster: `#1206`, `#1203`, `#1182`.
5. Pipeline cluster: `#1200`, `#1191`, `#1179`.
6. Obligations cluster: `#1198`, `#1188`.
7. LSP cluster: `#1207`, `#1186`.
8. Advisory artifact cluster: `#1189`.

## Current status

Visibility has been restored for the recoverable PRs. The reopened PRs are not
approved for merge and are not approved for closure. Each needs an evidence pass
from current `main`.

The PRs whose head refs are gone (`#1168`, `#1214`, `#1231`) require replacement
proof or recreation from the saved head SHA if their useful work is not already
represented by named merged commits.

## Boundary

This audit does not publish 0.3.2, does not run witnesses, does not add
automatic comments, does not edit source code through unsafe-review, and does
not make proof, UB-free, Miri-clean, site-execution, calibrated
precision/recall, or policy-ready claims.
