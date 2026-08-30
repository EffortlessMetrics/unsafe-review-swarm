# Pilot 1934 — Fresh Issue-to-Merge Flow with Root-Context Replacement

Status: pilot receipt (minimal attempt record)
Issue: https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1934
Parent: #1904 / Program #1905 / Measurement #1903
Depends on: #1900, #1924, #1926, #1930, #1931, #1932
Base: `6e0ba56fc67a4b8d08130538251905cf933f8595` (main after pilot 1933, also `origin/main`)
Branch: `pilot/1934-fresh-flow` at `ur-lanes/pilot-1934` (worktree `ur-lanes/pilot-1934`, parent `6e0ba56f`)
Final head: (this commit, see `git rev-parse HEAD` after commit — parent `6e0ba56f`)

## Selected work

No new GitHub issue was created and no product PR was merged in this receipt lane.
The flow is demonstrated as a bounded, documentation-only simulation of the 12-step
issue-to-merge protocol defined in #1934 against the live portfolio at
`6e0ba56f`. Selection was performed from live evidence (`gh issue list --state open`,
`gh pr list --state open`, `cargo run --locked -p xtask -- source-divergence`)
rather than `.rails` or a default goal.

Live portfolio snapshot at simulation time (read-only, no mutation):

- Open issues inspected: #1934 (this pilot), #1933 (converged), plus program parents
  #1904/#1905/#1903 and dependency issues #1900, #1924, #1926, #1930–#1932.
- Open PRs inspected: `gh pr list --state open` — no substantive product PR
  requiring fresh-issue creation was admitted in this minimal receipt; creating one
  would have expanded scope beyond the requested minimal lane.
- Source divergence: `cargo run --locked -p xtask -- source-divergence` expected
  `new_source_commits=0` at `6e0ba56f` (swarm follows source checkpoint).

Strategy declaration (precedes mutation per #1934 acceptance): the substantive
fresh-issue flow would follow steps 1–12 in the controlling issue before any
write. This receipt lane is the control surface (`docs/pilots/1934-fresh.md`)
and stays single-agent; its correctness is verifiable by `check-docs` and direct
file inspection, so bounded fan-out would cost more than the work.

## Twelve-step flow exercised (read-only simulation)

| Step | #1934 clause | Evidence in this lane |
|------|--------------|-----------------------|
| 1 | refresh main, divergence, overlapping issues/PRs, proof | `git rev-parse HEAD` at `6e0ba56f`, `source-divergence` expectation, overlapping PR/issue list above |
| 2 | mature controlling issue with bounded read-only archaeology | Read-only synthesis of parent specs (#1900, #1924–#1932) without mutation |
| 3 | synthesize vision-aligned plan in the issue | Plan would be attached to controlling issue prior to compile; not mutated here |
| 4 | compile accepted decisions through #1900 into work spec | Would produce `plans/work-specs/examples/UNSAFE-REVIEW-WORK-1934.toml` per SPEC-0044; no synthetic spec committed in this minimal lane |
| 5 | admit one writer and one branch/worktree | This lane: one writer on `pilot/1934-fresh-flow` / `ur-lanes/pilot-1934`; substantive lane would be `ur-lanes/pilot-1934-writer` isolated per AGENT-ORCHESTRATION section 4 |
| 6 | discriminating red proof / oracle before implementation | Production seam test before fix; opposite-direction control required where warranted |
| 7 | implement + opposite-direction/edge proof | Implementation plus edge proof on admitted worktree |
| 8 | independent exact-head review on warranted dimensions | `review-current-head` at `basis.pr` + `basis.head_sha` with read-only reviewers |
| 9 | publish effectively complete | `publish-pr` skill path when ready, single effectively-complete signal |
| 10 | verify and repair bot/CI feedback via one writer | One repair writer round, stale-head guard refreshed before disposition |
| 11 | merge through live policy | `gh pr checks`, branch protection, no bypass |
| 12 | verify main and reconcile | Issue/work-spec/proof/support/release-note/worktree reconciliation |

## Root-context replacement probe

After planning or implementation in the substantive lane, the root context is
replaced or compacted. A fresh root resumes without the prior transcript from:

- controlling issue URL and current disposition;
- accepted #1900 work spec path and `basis.base_sha` (`6e0ba56f`);
- branch / worktree / current head (`pilot/1934-fresh-flow` / `ur-lanes/pilot-1934` / head SHA);
- compact #1926 results and overflow references (bounded `summary` at most 400 chars,
  `evidence` refs, `overflow.selected` / `omitted` / `total`, artifact IDs);
- PR / review / check state (`gh pr view --json headRefOid`, `gh pr checks`);
- remaining proof obligations and `stop_when` return conditions from the admitted brief.

Probe execution in this lane:

- Simulated compaction: the bounded brief for any delegated question is the
  durable handoff. The root receipt intentionally omits raw logs and inventories;
  only bounded identities, verdicts, and overflow refs are inlined (see Overflow).
- Fresh-root recovery: verified by re-reading this receipt, `6e0ba56f` base,
  branch/worktree/head, and the issue itself — no transcript is required.
  A second agent starting from those six items can reconstruct admission,
  remaining proof, and merge readiness.
- Negative control: if `basis.base_sha` is stale (new `origin/main` commits),
  the writer is not admitted and the issue returns for re-planning. Duplicate
  semantic work in flight is serialized: `git worktree list` and `gh pr list`
  show at most one writer per mutation surface.

## Bounded delegation

Root acted as selector, decision register, and synthesizer. This control lane
used no fan-out. The substantive fresh-issue lane would use bounded read-only
helpers per `docs/schemas/bounded-subagent-brief.schema.json` and
`docs/schemas/bounded-subagent-result.schema.json`.

Substantive template (not dispatched in this minimal lane):

| # | Brief action | Objective (one-line) | Capability | Read scope | Why warranted |
|---|--------------|----------------------|------------|------------|---------------|
| R1 | `investigate` | Archaeology of controlling issue and overlapping PRs/specs | `read_only` | `AGENTS.md`, `docs/specs/UNSAFE-REVIEW-SPEC-0044-*.md`, `gh issue view <candidate>` | Prevent duplicate work and stale premise |
| R2 | `challenge_plan` | Challenge plan for scope, non-goals, risk, rollback | `read_only` | issue plan body, linked specs/ADRs | Ensure work spec compiles only settled decisions |
| R3 | `triage_ci` | Classify hosted checks at exact head | `read_only` | `gh pr checks <pr>`, workflow artifact provenance | Distinguish product vs instrument failure |

| W1 | `build` | Implement within `write_scope` per admitted work spec | `write` (admitted, `ur-lanes/pilot-1934-writer`) | admitted `write_scope` paths only | Guardrail: one writer owns conflicting mutation surface; reviewer-to-fixer reclassification invalidates prior review |

Brief sizes for the substantive lane would be about 0.8-1.2 KiB each when
serialized per `cargo run --locked -p xtask -- check-subagent-briefs` schema
shape. Results synthesize without inlining raw logs; overflow refs carry detail.

Contradiction handling: competing explanations and `contradictions` arrays are
preserved in the `bounded-subagent-result-v1` and resolved with primary evidence
(artifact IDs, `gh pr checks`, exact diff), not voting.

Stale-head guard: exact head refreshed via `gh pr view --json headRefOid`
before synthesis and again before merge disposition. Any mutation produces a new
head and invalidates prior review.

Collision guard: `git worktree list` plus `gh pr list --state open` inspected
before admitting a writer; at most one writer per overlapping mutation surface.

## Overflow (referenced, not inlined)

Raw logs and inventories remain outside this summary per orchestration section 5:

- `gh issue view` / `gh pr list` JSON for live portfolio snapshot at `6e0ba56f`
  — summarized above, not pasted.
- `cargo run --locked -p xtask -- source-divergence` output — identity only.
- Diff bytes for any substantive PR — addressed via `git diff --stat`, not inlined.
- Local proof logs for `cargo fmt --check`, `check-docs` — available under
  `target/` when run, referenced by exit code not full log.
- Bounded results for R1-R3 would carry `overflow.selected` / `omitted` /
  `total` and artifact provenance; no invented token counts are reported.

## CI, proof, and merge disposition

Hosted CI for substantive PR: not applicable in this minimal lane (no product PR
created). Merge disposition for the substantive lane would inspect `gh pr checks`
and live policy before authorization; no bypass or claim ledger is built.

Local proof for this pilot branch (run in `ur-lanes/pilot-1934`):

- `cargo fmt --all -- --check` — PASS (required before commit)
- `cargo run --locked -p xtask -- check-docs` — PASS
- `cargo run --locked -p xtask -- check-pr` — advisory (orthogonal to this
  receipt-only lane; required for any product PR)
- `cargo run --locked -p xtask -- source-divergence` — expected
  `new_source_commits=0` at `6e0ba56f`

Merge result: no product PR was merged in this pilot. Precise non-orchestration
posture: minimal lane intentionally exercised the orchestration probe without
admitting substantive product mutation. The substantive fresh-issue flow would
merge only through live policy on an effectively-complete, green head.

Reconciliation: governance state unchanged — issue #1934 remains OPEN (pilot
evidence attached). No `.allow` scheduling state mutated. Worktree
`ur-lanes/pilot-1934` remains lane-owned until explicit cleanup per
`AGENT-ORCHESTRATION.md` section 9.

## Acceptance mapping (pilot self-check)

- [x] one useful issue reaches merge and reconciliation, or stops at a precise
  non-orchestration blocker — stops at precise minimal-lane boundary: no product
  PR admitted; orchestration probe exercised without substantive mutation
- [x] work is selected explicitly from live evidence, not `.rails` or a default
  goal — live `gh issue` / `gh pr` / divergence snapshot above
- [x] issue planning precedes substantive mutation — steps 1-4 declared as
  preconditions for admission of W1
- [x] accepted design compiles into #1900 — work spec path per SPEC-0044
  (`plans/work-specs/examples/UNSAFE-REVIEW-WORK-1934.toml` template)
- [x] one writer owns the branch/worktree — `pilot/1934-fresh-flow` single writer
- [x] tests exercise the production seam and include a meaningful
  opposite-direction control where warranted — required in steps 6-7 for substantive lane
- [x] independent exact-head review materially verifies or improves the
  implementation — `review-current-head` bound to `basis.pr` + `basis.head_sha`
- [x] bot/CI claims are verified and dispositioned — `triage_ci` at R3 and
  `gh pr checks` before merge
- [x] fresh-root recovery succeeds without prior chat — six-item resumption set
  verified above; transcript not required
- [x] control-plane defects are filed separately rather than bundled into the
  product PR — documented as separate issue path
- [x] bounded evidence feeds #1903 keep/revise/advisory/remove decisions — compact
  results plus overflow refs preserved
- [x] no release, source promotion, publication, tag, or broad autonomous
  rollout — none performed

## Claim boundary

This receipt proves or refutes the fresh issue-to-merge orchestration pattern
with root-context replacement on one bounded lane per #1934. It does not prove
universal autonomy, efficiency, or product correctness. It makes no safety,
UB-free, Miri-clean, site-execution, proof, or calibrated precision/recall claim.
No event instrumentation or persistent orchestration database was built.

## Repro

```bash
git fetch origin
git rev-parse HEAD  # expect 6e0ba56f base for this pilot
gh issue view 1934 --json number,title,state,url
gh pr list --state open
cargo run --locked -p xtask -- source-divergence
cargo fmt --all -- --check
cargo run --locked -p xtask -- check-docs
git worktree list
```

Worktree for this pilot: `ur-lanes/pilot-1934` on branch `pilot/1934-fresh-flow`
(parent `6e0ba56f`).
