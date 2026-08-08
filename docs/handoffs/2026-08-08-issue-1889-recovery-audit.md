# Issue #1889 recovery audit

Date: 2026-08-08

This is the live-state recovery audit requested by issue #1889. It compares the
reported local-only commits with current `origin/main` at
`db787af5d998f55f7f8d1f0dfcc543cdb786809e`. It does not treat a reachable local
object, an old handoff, or a private branch as shipped capability.

## Source and swarm posture

- Swarm main: `db787af5` (`cli: name the fix on the first-hour input and policy failures`, PR #2012).
- Source main: `c25d6527` (`sync: remove residual RTK command guidance`, source PR #559).
- `source-divergence`: `new_source_commits=0`; the acknowledged checkpoint is already on swarm main.
- Publication, source promotion, tags, GitHub Release, and public `v1` remain outside this audit.

## Reported slice disposition

| Reported commit | Reported slice | Object exists locally | On current main | Live disposition |
| --- | --- | --- | --- | --- |
| `56fa9485` | action-first `pr` front panel | yes | no | The effective zero-config `pr` entrypoint and count/action fixes are already represented by merged PRs #2001, #2005, #2008, #2010, and #2012. The remaining one-screen compression in #1884 is still an open UX follow-up, not a stranded commit to cherry-pick. |
| `808b6dec` | preview-only adoption init | yes | no | The focused `baseline init --dry-run` behavior was recreated and merged as PR #1995. The broader top-level guided `unsafe-review init` remains a separate open scope in #1885. |
| `0f4a1866` | paired ub-review pin and policy ledger | yes | no | The effective parity repair and current action pins are represented by PRs #1943 and later dependency merges. The local commit is not a candidate input; `workflow-pin-sync` is now the durable repair/check path. |
| `15ade3f8` | tokmd five-preset producer acceptance | yes | no | The dotted `tokmd.packets/v1` producer schema field is on main via `daeb33ba` (PR #1863). Five-preset consumer acceptance remains unproven and stays open in #1857. |
| `f2d2cbb0` | capped holdout status | yes | no | The effective holdout disclosure work is represented by the merged holdout and CLI slices, including PRs #1996–#2000 and #2008. Do not transplant the local commit. |
| `2d35432b` | portable atomic receipt | yes | no | The receipt is represented by the merged dogfood baseline work. Treat the local commit as historical evidence only. |

## Focused next lanes

1. Keep #1884 open for the remaining action-first presentation decision; its
   count-parity prerequisite is merged, but the full UX compression is not.
2. Keep #1885 open for the broader guided top-level adoption flow; PR #1995
   completed only the narrower `baseline init --dry-run` slice.
3. Continue #1857 with a fresh current-main tokmd producer/consumer receipt,
   using the canonical tokmd schema rather than guessed field names.
4. Use #1890 for the pinned cargo-allow current-main evidence receipt and
   classification needed by the release cutline.
5. Treat #1889's reported commits as recovered, superseded, or routed above;
   none should be cherry-picked wholesale into a release candidate.

## Claim boundary

This audit proves only the stated object/reachability and current-main routing
observations. It does not prove analyzer accuracy, safety, UB-free status,
Miri cleanliness, site execution, installed-product readiness, or publication
authorization.
