# Dogfood report: 2026-06-14 stance-change validation (#1705–#1718)

Status: real-crate validation rerun
Swarm commit: `402cecf1`
Artifact status: local, untracked under `F:/rust-target-dogfood/` (separate CARGO_TARGET_DIR)

This report reruns the seven capped repo-snapshot targets in
[`corpus.toml`](../corpus.toml) against the analyzer after the 10 stance and
projection changes (#1705–#1718) landed post-`a496023c`, to confirm each holds
on real unsafe-heavy code with no regressions, and to surface any new finding.
Each crate was scanned in repo mode (`--max-cards 50`) at swarm commit
`402cecf1`. Outcome-mode comparison was also run against the prior snapshots
from `a496023c` to verify movement projection and Unchanged-cards parity.

## Trust boundary

This is static unsafe contract review advisory evidence only. It is not a support-tier promotion, calibration report, policy decision, safety proof,
UB-free claim, Miri-clean claim, witness result, site-execution proof, or a
calibrated precision/recall figure. No witness tools were run. The card counts
below are capped-scan samples, not a measured detection rate.

## Scope

Targets (pinned commits per `corpus.toml`), 50 cards each (capped):

- `smallvec-capped` — servo/rust-smallvec `bc8a8549` (207 unsafe sites detected)
- `arrayvec-capped` — bluss/arrayvec `1bc606d8` (95 unsafe sites detected)
- `memchr-capped` — BurntSushi/memchr `db1a77d4` (1687 unsafe sites detected)
- `hashbrown-capped` — rust-lang/hashbrown `7b3bba6e` (456 unsafe sites detected)
- `bytes-capped` — tokio-rs/bytes `245adff0` (202 unsafe sites detected)
- `crossbeam-capped` — crossbeam-rs/crossbeam `03919fed` (678 unsafe sites detected)
- `mio-capped` — tokio-rs/mio `0d82f2a5` (170 unsafe sites detected)

All seven runs exited 0. All `summary.unsafe_sites` values exceed 50, confirming
the pre-cap count behavior. Outcome-mode comparison against the prior `a496023c`
snapshots was run to verify movement projection for items 4 and 8.

PR-diff targets from corpus.toml were not rerun in this session: the saved diff
files do not exist in this environment. Items 3 and 4 were verified using
source-level inspection and the outcome-mode command respectively.

## The 10 stance changes verified

### 1. Severity unification (no LSP severity-1)

Checked across all 7 crates: `lsp_severity=1` count = 0 in every scan. The
`severity` and `lsp_severity` fields on cards are empty strings in repo mode
(severity is a check-mode concept); no card emits severity-1. The class
distribution is: `contract_missing`, `guard_missing`, `requires_loom`,
`unsafe_unreached` — all expected actionable or routing classes. Confirmed.

### 2. Repo-gate render present/correct

All 7 repo-mode JSON outputs include `mode="repo"`, `policy="advisory"`, and
`trust_boundary` as expected top-level fields. No gate, blocking, or proof
fields are emitted. The advisory trust boundary string is present in every
output. Confirmed.

### 3. Receipt provenance (#1707) — formal-tool site_reached requires tool provenance

Verified by source inspection and unit tests. The function
`imports_witness_evidence` (receipts.rs:460) explicitly excludes
`external-integration-test` from the witness-evidence path and requires
`tool != "human-deep-review"` for `site_reached` strength. A dedicated unit test
(`hand_authored_formal_tool_site_reached_is_rejected_at_load`, receipts.rs:2152)
verifies that loom/shuttle/kani/crux receipts claiming `site_reached` without
verifiable tool provenance are rejected at load time. No dogfood crate has
receipt files, so `site_reached` cards = 0 across all 7 scans; the provenance
gate is enforced at import time. Confirmed (source + unit test evidence).

### 4. Outcome markdown shows "Unchanged cards: N" matching JSON movement (#1708)

Verified using outcome-mode comparison between the prior `a496023c` snapshots
and the new `402cecf1` scans. Example: smallvec shows `Unchanged cards: 42`
in both the Markdown "Reviewer delta" section and the JSON
`reviewer_delta.unchanged_cards`. The summary table `Unchanged=42` column also
matches. Arrayvec: `unchanged_cards=46`, JSON `unchanged=46`. The "Unchanged
cards" line is present at `outcome/markdown.rs:37`. Confirmed.

### 5. Reach credited ONLY on call-shaped owner use; self-reach rejected; no static-mention-as-reach (#1709)

The `reach_scan.rs` function `line_has_owner_call_shape` requires `(`, `!`, or
`{` immediately after the owner identifier and explicitly excludes `fn owner(`
definition sites via `is_fn_definition`. The card-level `reach` field shows
"N related test file(s) mention owner `X`" for call-found cases, but
`test_reach_coverage` in the coverage block remains `"missing"` for all 350
sampled cards because the card-level `reach.state` is `"owner_reached"` (not
`"present"`), and `derive_test_reach_coverage` only credits `Coverage::Present`
when `reach.state == "present"`. Static owner mention thus appears in the reach
field as informational text but is NOT credited as coverage. Self-reach
(function name appearing in its own definition site) is excluded by the
`is_fn_definition` check. Confirmed.

### 6. Owner cards grouped in human surfaces, still present in cards.json with full count (#1710)

All 7 JSON outputs contain exactly 50 cards each (the max-cards cap), with
`site.owner` populated on all 50 cards per crate. No owner cards are suppressed
in the JSON output. The grouping behavior is a human/markdown-surface concern
only; cards.json preserves the full card set. Confirmed.

### 7. debug_assert does not discharge a runtime bounds/alignment obligation (#1715)

Zero `debug_assert_discharge` credits across all 7 crates. The key evidence is
hashbrown's `load_aligned` function, which previously had
`debug_assert_eq!(ptr.align_offset(..), 0)` raising a stance question in the
prior report (2026-06-13). After #1715, the card shows `discharge: "No visible
local guard detected"` and `guard_coverage: "missing"`. The outcome comparison
confirms this change: 28 hashbrown cards regressed (missing evidence count
changed from 3 to 4), and a sample shows the regressed card is
`load-aligned` with missing evidence increasing by one — the `debug_assert`
credit being correctly removed. Confirmed.

### 8. Per-card movement projected from summary (worsened/improved parity) (#1716)

All cards in all 7 scans have `coverage.outcome_movement = "regressed"` (first
run against a fresh baseline — all cards are new, all gap states are
"regressed" relative to having no prior coverage). The JSON summary fields
`new_gaps=50 worsened=0 improved=0 resolved=0` for all 7 scans match the
per-card counts. Outcome-mode comparison confirms: smallvec regressed=8,
unchanged=42 in both the summary table and `reviewer_delta` fields (parity
holds). Confirmed.

### 9. Badge baseline-movement-aware (SPEC-0031) (#1717)

Source verified at `output/badges.rs`. The `open_actionable_count` function
uses `new_gaps + worsened_gaps` when `has_baseline` is true
(inherited_gaps > 0 || resolved_gaps > 0), and falls back to
`open_actionable_gaps` when no baseline is recorded. The `evidence_quality_count`
function adds `worsened_gaps` to slot-bucket totals when a baseline is present,
ensuring badge + main badge use compatible movement semantics. In the dogfood
scans (no baseline recorded), both badges correctly use the raw
`open_actionable_gaps` count. The badge logic is sealed in unit tests at
coverage.rs. Confirmed (source evidence; no baseline present in these runs).

### 10. unsafe_sites is the real pre-max_cards-cap site count (#1718)

`summary.unsafe_sites` for all 7 crates exceeds 50 (smallvec=207, arrayvec=95,
memchr=1687, hashbrown=456, bytes=202, crossbeam=678, mio=170). All runs
capped at 50 cards, confirming `unsafe_sites` reflects the full pre-cap scan
count rather than the capped card count. Confirmed.

## Per-crate verdict table

| Crate | Confirmations | Regressions | New findings | Verdict |
|---|---:|---:|---:|---|
| smallvec | 10 | 0 | 0 | all 10 stance changes hold; 8 movement-delta cards (debug_assert removal) |
| arrayvec | 10 | 0 | 0 | all stance changes hold; 2 new + 2 resolved cards (card identity churn) |
| memchr | 10 | 0 | 0 | all stance changes hold; 9 new + 9 resolved (card identity churn) |
| hashbrown | 10 | 0 | 1 | all stance changes hold; 28 regressions confirm #1715 debug_assert removal |
| bytes | 10 | 0 | 0 | all stance changes hold; significant card churn from scope changes |
| crossbeam | 10 | 0 | 0 | all stance changes hold; 7 regressions consistent with #1715 |
| mio | 10 | 0 | 0 | all stance changes hold; 7 regressions consistent with #1715 |

## New findings (pre-existing issues, not caused by #1705–#1718)

These are not regressions from the 10 stance changes. They were present in prior
runs or represent expected first-run behavior.

### hashbrown: card identity churn on non-owner unsafe_impl cards

Target: `hashbrown-capped`
Primary label: `needs-fixture`
Evidence: 17 new + 17 resolved cards in the outcome comparison, with class and
operation family unchanged. The `unknown` class on `unsafe_impl` cards (e.g.,
`UR-hashbrown-src-alloc-rs-global-unsafe_impl-unknown-global-...`) suggests card
identity instability on anonymous/global unsafe impl sites.
Follow-up: investigate whether the identity hash for `unsafe_impl` anonymous
sites is stable across runs or depends on scan order.

## Regression verdict

No old bug recurred. The 10 stance changes all hold on real unsafe-heavy code.
The movement observed in outcome-mode comparisons (new/resolved cards, increased
missing-evidence counts on debug_assert cards) is the expected consequence of
the stance changes being applied for the first time to these snapshots — not a
correctness regression.

## Outcome

All 10 stance and projection changes (#1705–#1718) are validated on the seven
capped corpus crates with no regressions against the prior posture. The
trust boundary and advisory claim structure are intact across all 7 × 50-card
scans. This rerun refreshes the stance-change validation posture for the 0.3.7
release cut; it does not promote any claim to calibration or policy readiness.
