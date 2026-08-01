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

`baseline init --dry-run` previews this same plan without writing the ledger or
snapshot. The preview is read-only and may use `--format json` for automation;
human and JSON output carry the same card identities, locations, proposed paths,
and advisory trust boundary. The default `baseline init` apply behavior is
unchanged and remains explicit.

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
improved   = baseline cards whose evidence coverage improved (pure improvement:
             at least one slot advanced and no slot regressed since the baseline)
resolved   = baseline cards no longer open (site removed or repaired)
inherited  = baseline_known cards still open and unchanged
```

**Precedence rule**: worsened > improved > inherited.  If any slot regressed the
card is counted worsened.  If no slot regressed but at least one slot improved
the card is counted improved.  Otherwise the card is inherited/unchanged.
A mixed up-and-down movement is counted worsened, not improved.

**Trust boundary for `improved`**: an improved card is still advisory, still
open, and still present.  It is NOT resolved, NOT safe, NOT UB-free, NOT
Miri-clean, and NOT a site-execution claim.  `improved` means evidence coverage
got better — the author added a safety contract, guard, test-reach signal, or
witness receipt — but the card persists.  It is surfaced as positive movement so
authors are rewarded for adding evidence without the tool being silent about it.

On a diff-scoped run (`first-pr`, `check --base/--diff`), `new` and `worsened`
are constrained to cards attributable to the diff (changed-line sites), so a PR
is judged on what it changed. On a repo-mode run, movement is measured against
the whole baseline floor. A repo with 91 inherited gaps and a PR that adds 2 new
and worsens 1 but improves 3 reports `new=2 worsened=1 improved=3 inherited=88`,
not "94 failures."

The movement summary is the shape consumers read (and the basis for the
`unsafe-review-gate.json` manifest in UNSAFE-REVIEW-SPEC-0034):

```json
{ "baseline": "target/unsafe-review/baseline.json",
  "new_gaps": 2, "worsened_gaps": 1, "improved_gaps": 3,
  "resolved_gaps": 3, "inherited_gaps": 88,
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

Each card in the policy report carries a `baseline_state` field and a
`policy_status` field with distinct roles:

- `baseline_state` — the canonical 5-value coverage-movement vocabulary
  (`new`, `worsened`, `inherited`, `resolved`, `unknown`) projected from
  `CoverageBlock::derive` with snapshot-slot movement applied. This is the
  same value the `json` and `agent` surfaces project (SPEC-0030 §single-truth,
  canonical unification). Advisory only; no proof, UB-free, or Miri-clean claim.

- `policy_status` — the policy-classification vocabulary
  (`new_gap`, `baseline_known`, `suppressed`, `non_actionable`) that reflects
  how the card was matched against the baseline and suppression ledgers. This is
  the field consumers use for policy-enforcement decisions.

A `Suppressed` card has `baseline_state = "unknown"` (CoverageBlock does not
assign baseline posture to suppressed cards) and `policy_status = "suppressed"`.
A `NonActionable` card has `baseline_state = "unknown"` and
`policy_status = "non_actionable"`. The `policy_reason` field carries the
human-readable explanation for the `policy_status` classification.

### Resolved and stale baseline entries

When a baseline-known card no longer appears (the unsafe site was removed or
repaired), the entry is reported as `resolved` in the policy report so baselines
can be pruned. Expired baseline `review_after` dates are surfaced by
`unsafe-review baseline status` as the `review_due` bucket (see below), not
auto-removed. The existing suppression expiry behavior (UNSAFE-REVIEW-SPEC-0010)
is unchanged.

### Baseline health and refresh preview (issue #1893)

`unsafe-review baseline status [--root .] [--format human|json]` is a read-only
ledger-health report. It classifies every baseline ledger entry, plus every
currently open actionable card the ledger does not represent, into one of ten
buckets, reusing the movement/identity signals defined above (no second movement
model, no change to exact card-id matching):

```text
active_unchanged                baseline-known, still open, coverage unchanged
active_improved                 baseline-known, still open, coverage improved
active_worsened                 baseline-known, still open, coverage regressed
resolved                        ledger entry's card no longer appears
review_due                      ledger entry's review_after has passed
snapshot_missing_or_invalid     no usable coverage-snapshot floor for this card
duplicate_or_conflicting_entry  card_id appears more than once in the ledger
suppression_overlap             card_id is baseline-known and actively suppressed
identity_unmatched              bad card_id shape, or the entry is otherwise
                                 structurally invalid (see below)
new_unbaselined                 open actionable card the ledger does not represent
```

`suppression_overlap` only fires for a baseline entry covered by a **currently
active (non-expired)** suppression entry, reusing the exact same expiry predicate
as `policy report`'s `expired_suppressions` (one expiry model, not two): an
expired suppression is already surfaced by `policy report` through
`expired_suppressions` (there is no expired-suppression bucket among the ten
`baseline status` buckets), so folding it into `suppression_overlap` too would
double-report the same stale entry under two labels. `new_unbaselined` never includes a card covered by any
suppression entry, active or expired — the core analyzer classifies such a card
`Suppressed` (not actionable) before `baseline status` runs at all, so it is
excluded by the existing analyzer classification, not by a second expiry check
inside this module.

`identity_unmatched` also covers ledger entries that are structurally invalid in
other ways — missing/empty `owner`/`reason`/`evidence`, or a missing or malformed
`review_after` date (schema-required on every entry) — instead of a broken entry
silently falling through to `active_unchanged`/`resolved` and looking healthy.
There is still exactly ten buckets; a structurally invalid entry is folded into
the existing `identity_unmatched` bucket rather than adding an eleventh.

A baseline ledger entry that fails the analyzer's own *strict* per-entry
validation (used by every other command, e.g. `policy report`) would otherwise
abort the whole repo-wide card scan before `baseline status` could report
`identity_unmatched` for the offending row — defeating the corrupt-ledger-
diagnosis purpose for exactly the class of problem it exists to catch.
`baseline status` degrades instead: it still succeeds, still flags the bad
entry `identity_unmatched`, and sets `card_scan_error` on the report (surfaced
as a warning in both human and JSON output) — in that state `resolved` means
"the repo-wide card scan could not run", not "confirmed gone." Every other
scan failure (a genuinely unparseable ledger, a broken suppression ledger, a
source-scan error) still fails `baseline status` outright.

Classification precedence puts every entry-only bucket ahead of the
scan-dependent `resolved` bucket: `review_due` (a past `review_after`, derivable
from the entry and `today` alone) is evaluated before `resolved`, so a degraded,
scan-unavailable run cannot mask a due entry as `resolved` — inflating that count,
flipping the ledger to "fully healthy" and hiding the `review_due` signal from
the `baseline status` report it belongs in.

Human and JSON output project from the same report, so both always report
identical bucket counts and entry identities. `unsafe-review baseline refresh
--dry-run [--root .] [--out dir]` builds a deterministic per-entry action plan
from the same classification — `keep`, `update_snapshot`, `mark_resolved`,
`advance_review_after`, `add_new_debt`, or `conflict`. It leaves repository
policy, source, and snapshot state unchanged, writing a plan artifact only when
`--out` is explicitly given; `--dry-run` is required and there is no apply mode
(see Non-goals). `advance_review_after` and `add_new_debt` are always flagged as
requiring a separate, explicit decision, never auto-applied; no resolved entry is
silently removed by this preview. When `card_scan_error` is set (the degraded,
scan-unavailable state above), every scan-dependent `resolved` entry is planned
as `conflict`, never `mark_resolved`: an unverifiable disappearance must require
human resolution and must never be presented as a confirmed deletion. Buckets
that do not depend on the card scan (`identity_unmatched`,
`duplicate_or_conflicting_entry`, `suppression_overlap`) keep their normal
`conflict` action.

The `pr`/`first-pr` brownfield-baseline handoff points to `baseline status`
before suggesting `baseline init` again when a ledger already exists. It does
not invent a second movement model, change `policy_status`, or touch badges, the
gate manifest, or agent/LSP surfaces.

**Stance revision (issue #2004).** `pr` previously also surfaced a bounded
one-line warning when the ledger had any entry outside the
`active_unchanged`/`resolved` buckets. That warning is retired. Classifying the
ledger required a full repository scan (`Scope::Repo`, `max_cards: None`) on a
command whose actual review is diff-scoped, which measured ~235x the cost of the
identical `first-pr` run on this repository and was paid on every `pr` run by
exactly the repositories that had adopted baselines. What it bought was a
conditional adjective in front of a `baseline status` command the handoff above
already prints unconditionally and for free. The health report itself is
unchanged and still available on demand — `baseline status` is where the scan
belongs, because there the user has asked for it.

Neither entrypoint may reintroduce repository-scanning work to decide how to
word the front-door baseline pointer.

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
  calibrated precision/recall, or policy-readiness claim,
- implement a `baseline refresh` apply mode (issue #1893) — `--dry-run` is
  required and there is no way to write the previewed plan back to policy files
  in this spec; a future apply command would be separately approved, explicit,
  idempotent, and refuse to overwrite a changed ledger,
- add a fuzzy or structural identity matcher — `identity_unmatched` in
  `baseline status` reports when the exact-identity contract fails to match,
  it does not introduce a fallback matcher.

## Trust boundary

A no-new-debt pass means only that the change under review did not add open
actionable unsafe-review gaps above the recorded baseline. It is not a statement
that the changed code is memory-safe, UB-free, Miri-clean, or that any unsafe
site executed safely. Baseline entries are debt records, not safety records.
`baseline status` and `baseline refresh --dry-run` carry the same boundary: they
classify existing ledger/movement signals, and both leave repository policy,
source, and snapshot state unchanged. `baseline refresh --dry-run` writes a
plan artifact only when the explicit `--out` option is given.

## Proof obligations

- `cargo test -p unsafe-review-core policy` — baseline/suppression exact match,
  new-debt set arithmetic, resolved/expired reporting, and the ten `baseline
  status`/`baseline refresh` health buckets (issue #1893).
- `cargo test -p unsafe-review-cli baseline` — `baseline init` / `baseline add`
  / `baseline status` / `baseline refresh` argument parsing, including the
  required `--dry-run` refusal.
- `cargo test -p unsafe-review --test e2e baseline` — `baseline status` human/JSON
  parity, and `baseline refresh --dry-run` writes-nothing and determinism.
- `cargo test -p unsafe-review --test e2e neither_entrypoint_scans_the_repository_for_baseline_ledger_health`
  — the retired `pr`-only health warning (issue #2004): both entrypoints still
  print the free `baseline status` pointer, neither classifies ledger health, and
  their brownfield-baseline blocks are byte-identical.
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
