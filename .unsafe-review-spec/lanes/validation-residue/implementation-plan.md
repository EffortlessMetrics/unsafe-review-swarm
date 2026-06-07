# Validation-residue lane: from shipped 0.3.4 to road-worthy instrument

## Scope

Convert the 0.3.4 consumer-validation findings into a finished, road-worthy
coverage instrument. The build-out goal (SPEC-0029 through SPEC-0035) shipped in
0.3.4 and was proven load-bearing in both intended consumers (ub-review
structured ingestion + inline comments; Bun brownfield baseline/no-new-debt —
see `docs/handoffs/2026-06-07-0.3.4-consumer-validation.md`). Validation
converted "shipped" into a concrete residue of honesty and loop-completion
gaps. This lane burns that residue down and ends at the next source promotion.

This lane is issue-driven by design: the filed validation issues are the work
items. No broad analyzer expansion happens inside this lane.

## Operating rule

One issue -> one PR-sized slice -> proof commands -> merge. Each slice keeps
ReviewCard as the single projected truth and preserves the advisory trust
boundary (no proof, UB-free, Miri-clean, site-execution, calibrated-precision,
or policy-ready claim; no witness execution, posting, source edits, or default
blocking).

## Phase 1 — instrument honesty at consumer scale

The instrument's numbers must mean what they say on foreign repos at Bun scale.

1. **#1543 `baseline init/add --out` wrote into `--root/policy/`** — DONE
   2026-06-07 (PR #1551, merged `b368f112`). Snapshot now derives as sibling
   `<ledger-stem>-snapshot.toml`; scanned repo stays read-only; rule codified
   in SPEC-0030.
2. **#1545 `--max-cards` caps silently.** A capped scan exits 0 with no status
   artifact, so consumers cannot distinguish "complete, N cards" from "cap hit
   mid-file". Emit a status artifact (and a `capped` signal in the report)
   mirroring the existing `--timeout-seconds` status path. Proof: e2e asserting
   the status artifact and capped signal; existing timeout-path tests stay
   green.
3. **#1552 repo self-scan walks nested git checkouts.** Nested worktrees or
   vendored checkouts under the scan root inflate posture counts (observed 3x:
   576 -> 1728). The scanner should stop at directories carrying a `.git`
   entry other than the scan root. Proof: fixture or e2e with a nested
   checkout; the in-place `xtask` badge tests pass again on polluted local
   checkouts.
4. **#1546 per-file scan cost is undocumented at scale.** Document observed
   per-file cost and large-repo scoping guidance (`--max-cards`,
   `--timeout-seconds`, path scoping) so first runs on big repos are planned,
   not surprising. Docs-only; aligns with SPEC-0035 diagnosability.

## Phase 2 — close the orchestrator loop

5. **#1542 repair-queue applicable edits.** Repair-queue entries today carry
   intent but not applicable edits, so ub-review's inline `suggestion`
   facility (already wired and waiting on their side) has nothing to render.
   Emit bounded, explicitly-labelled candidate edits in the repair-queue
   projection. Trust boundary: edits are proposals projected from ReviewCards;
   unsafe-review never applies them, never posts them, and labels them
   non-verified. Proof: repair-queue schema goldens + e2e; consumer smoke via
   the pinned ub-review integration.

## Phase 3 — deliberate breadth decision (a gate, not work)

**#1544 stable-byte v1 breadth.** Validation showed stable-byte v1 misses the
canonical Bun seam (JS-owned buffer raw pointer held across a JS promise
resolution, in safe Rust above the unsafe block). Analyzer breadth is a new
lane that starts with a proposal/spec, not code. Exit criterion for this lane:
either a drafted breadth proposal linked from the index, or an explicit parked
disposition recorded on the issue. Either outcome closes phase 3.

## Release cutline

When phases 1 and 2 are merged green on swarm, promote to source as the next
0.3.x through the standard ceremony (history-preserving via shipper, local
gates first, publication receipts recorded in a handoff). Phase 3's decision
can ride along as a disposition note; its implementation cannot.

## Do not block on

- Analyzer breadth (#1544 implementation) — separate future lane.
- Floor-time/cost receipts — owned by the workflow/ub-review layer, not
  gate.json.
- Editor-extension and marketplace lanes — independent tracks.
- Dependabot/automation PR queue — routine hygiene, parallel to this lane.

## Proof commands

```bash
cargo test --workspace --locked
cargo run --locked -p xtask -- check-pr
cargo run --locked -p xtask -- check-advisory-artifacts <dir>   # for artifact-shape slices
cargo run --locked -p xtask -- source-divergence
git diff --check
```

Known caveat until #1552 lands: run badge-affected gates (`check-pr`,
`cargo test -p xtask public_*`) from a clean worktree of `origin/main` when the
local checkout contains nested worktrees.
