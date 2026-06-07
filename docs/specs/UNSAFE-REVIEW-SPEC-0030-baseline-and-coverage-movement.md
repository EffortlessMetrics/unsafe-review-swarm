# UNSAFE-REVIEW-SPEC-0030: Baseline and coverage movement

Status: proposed
Owner: product / cli
Created: 2026-06-06
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked ADRs:
- none
Linked plan:
- plans/0.2.0/implementation-plan.md
Linked issues:
- none
Linked PRs:
- TBD
Support-tier impact: coverage movement posture
Policy impact:
- policy/unsafe-review-baseline.toml
- policy/unsafe-review-suppressions.toml

## Problem

`unsafe-review` reports unsafe coverage gaps but cannot say what a change
*moved*. On a mature repository the first run surfaces every pre-existing unsafe
site, so any consumer that treats the full gap set as actionable is overwhelmed
and mutes the tool. `--policy no-new-debt` does not help: it is implemented as a
*zero-debt* check that counts every open actionable gap regardless of whether
the change introduced it, and the baseline ledger (exact counted card-id match)
exists but is empty and hand-entry only.

This is the keystone adoption unlock (UNSAFE-REVIEW-SPEC-0028, PR 3). The unit
must be **coverage movement relative to a baseline**, not a total gap count:

```text
new        gaps the change introduced
worsened   gaps whose evidence got weaker (a coverage slot regressed)
resolved   baseline gaps the change removed or repaired
inherited  pre-existing gaps the change did not touch
```

`unsafe-review` reports this movement as posture. It does not block by default;
the orchestrator (UNSAFE-REVIEW-SPEC-0028 boundary) decides whether `new` or
`worsened` movement should fail a gate. This is a coverage-movement instrument,
not a pass/fail gate. Movement reads the coverage slots defined in
UNSAFE-REVIEW-SPEC-0029.

## Behavior

### Baseline as the coverage floor

A baseline records the open actionable ReviewCard identities and their coverage
state at a known point, so movement can be measured against that floor.

`unsafe-review baseline init [--root .] [--out policy/unsafe-review-baseline.toml]`
captures the current open actionable cards as baseline entries. Each entry keeps
the existing required fields (`card_id`, `owner`, `reason`, `evidence`,
`review_after`) and is written with an honest default `reason`
("captured by `baseline init`; pre-existing debt, not reviewed as safe") and a
`review_after` date. `baseline init` never marks anything safe; it records that a
gap pre-existed.

The coverage snapshot is written as a sibling of the ledger, derived from the
ledger file name (`<ledger-stem>-snapshot.toml`). The default ledger path keeps
the canonical `policy/unsafe-review-baseline-snapshot.toml`; a custom `--out`
keeps both authored files together. Baseline authoring never writes into
`--root` when `--out` points elsewhere: the scanned repository stays read-only,
matching the advisory no-source-edits boundary.

`unsafe-review baseline add --card-id <UR-...-cN> --owner <name> --reason <text>
--evidence <text> [--review-after <date>]` adds or updates a single entry, so the
ledger does not have to be hand-edited as raw TOML.

A baseline entry classifies its card as `baseline_known` exactly as today; this
spec changes how the gate counts, not the existing exact-match ledger semantics
(UNSAFE-REVIEW-SPEC-0010).

### Coverage movement, reported as posture

Movement is computed against the baseline floor and reported, not enforced:

```text
new        = open actionable cards minus baseline_known minus suppressed
worsened   = baseline cards whose coverage slot regressed since the baseline
resolved   = baseline cards no longer open (site removed or repaired)
inherited  = baseline_known cards still open and unchanged
```

On a diff-scoped run (`first-pr`, `check --base/--diff`), `new` and `worsened`
are constrained to cards attributable to the diff (changed-line sites), so a PR
is judged on what it changed. On a repo-mode run, movement is measured against
the whole baseline floor. A repo with 91 inherited gaps and a PR that adds 2 new
and worsens 1 reports `new=2 worsened=1 inherited=91`, not "94 failures."

The movement summary is the shape consumers read (and the basis for the
`unsafe-review-gate.json` manifest in UNSAFE-REVIEW-SPEC-0034):

```json
{ "baseline": "target/unsafe-review/baseline.json",
  "new_gaps": 2, "worsened_gaps": 1, "resolved_gaps": 3, "inherited_gaps": 91,
  "policy": "advisory" }
```

### Optional gate hook (orchestrator-owned)

`--policy no-new-debt` is redefined as a convenience exit-code hook for callers
that want unsafe-review itself to signal: it exits nonzero iff `new` or
`worsened` movement is non-empty, and is a no-op pass when the baseline is empty
and nothing is new. This is a thin convenience over the posture report; the
authoritative decision to block belongs to the orchestrator
(UNSAFE-REVIEW-SPEC-0028 boundary). The previous zero-debt behavior (fail on any
open gap, ignoring baseline) is removed. Default remains advisory; no blocking.

### Policy report

`unsafe-review policy report` reports the four movement buckets with each card's
identity and changed-line attribution, separated from `suppressed` counts. It
remains advisory and changes no exit code by itself.

### Resolved and stale baseline entries

When a baseline-known card no longer appears (the unsafe site was removed or
repaired), the entry is reported as `resolved` in the policy report so baselines
can be pruned. Expired `review_after` dates are surfaced, not auto-removed. The
existing suppression expiry behavior (UNSAFE-REVIEW-SPEC-0010) is unchanged.

## Adoption flow

The intended brownfield onboarding is three commands, not 200 ledger edits:

```text
unsafe-review baseline init            # record today's debt as the floor
git add policy/unsafe-review-baseline.toml && commit
# from now on:
unsafe-review check --policy no-new-debt   # fails only when the diff adds debt
```

## Non-goals

This spec does not:

- implement blocking policy (`PolicyMode::Blocking` remains deferred per
  UNSAFE-REVIEW-SPEC-0010),
- claim baseline-known cards are safe, reviewed, UB-free, or Miri-clean — a
  baseline records that a gap pre-existed, nothing more,
- auto-prune or auto-edit ledgers (resolution and expiry are reported, the human
  decides),
- change the exact counted card-id (`UR-...-cN`) matching contract,
- introduce per-line suppression comments in source (that remains out of scope),
- post comments, run witnesses, edit source, or make any proof, site-execution,
  calibrated precision/recall, or policy-readiness claim.

## Trust boundary

A no-new-debt pass means only that the change under review did not add open
actionable unsafe-review gaps above the recorded baseline. It is not a statement
that the changed code is memory-safe, UB-free, Miri-clean, or that any unsafe
site executed safely. Baseline entries are debt records, not safety records.

## Proof obligations

- `cargo test -p unsafe-review-core policy` — baseline/suppression exact match,
  new-debt set arithmetic, resolved/expired reporting.
- `cargo test -p unsafe-review-cli` — `baseline init` / `baseline add` parsing
  and ledger round-trip; `--policy no-new-debt` exit codes for the
  pre-existing-debt-only, new-debt, and empty-baseline cases.
- `cargo test -p unsafe-review` — diff-scoped no-new-debt e2e on a fixture with
  pre-existing and PR-added gaps.
- `cargo run --locked -p xtask -- check-pr`.

## Machine check

Registered in `docs/specs/UNSAFE-REVIEW-SPEC-STATUS.md` and routed from
`docs/specs/UNSAFE-REVIEW-SPEC-START-HERE.md`; lifecycle and proof posture
validated by `cargo run --locked -p xtask -- check-spec-status` and
`cargo run --locked -p xtask -- check-docs`.
