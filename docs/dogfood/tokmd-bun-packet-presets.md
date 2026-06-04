# tokmd Bun Packet Presets

Status: future renderer requirements plus current first-pr packet input

This note records the Bun packet shapes that `tokmd` should eventually render
from unsafe-review manual candidates, ReviewCards, witness plans, receipt
audits, repair queues, and stable-byte seed ledgers. It is a requirements rail
only. The current `first-pr` lane writes `tokmd-packets.json` as formatting
input for imported manual candidates. It does not run tokmd, render packet
Markdown, run witnesses, edit source, post comments, or claim a candidate is
proved.

## Purpose

The Bun stable-byte burndown needs repeatable packet presets so scouts,
implementers, and maintainers can move from candidate evidence to a smallest
upstreamable PR without reformatting the same route, proof mode, fix boundary,
and stop line by hand.

Each preset must preserve the source of truth:

- analyzer ReviewCards stay ReviewCard-derived;
- manual candidates stay `source = manual`, `manual_candidate = true`, and
  `analyzer_discovered = false`;
- receipts attach external evidence only to their exact target;
- repair queues remain copy-only handoffs, not automatic repairs.

## Presets

### `bun-ub-handoff`

Audience: rust lane implementer.

Required sections:

- candidate or ReviewCard identity;
- stable-byte family and invariant at risk;
- safe JS caller route;
- Rust/native seam and file:line;
- proof mode and missing proof;
- current evidence with limitations;
- suggested fix boundary;
- PR aperture and stop line;
- test or witness target;
- do-not-touch list;
- ledger state and next action.

### `bun-ub-pr-body`

Audience: upstream maintainer.

Required sections:

- problem statement without UB-proof overclaiming;
- user-visible or invariant-level risk;
- smallest changed surface;
- compatibility behavior or Node/Bun oracle, when relevant;
- tests and external evidence receipts, if present;
- non-goals;
- exact claims not made.

This preset must be reviewable as a small PR body. It must not include broad
scout logs, unrelated sibling candidates, or source-route-only claims as sure
UB.

### `bun-ub-ledger-note`

Audience: Bun burndown ledger maintainer.

Required fields:

- seed or candidate ID;
- old ledger state and new ledger state;
- evidence or PR receipt that justifies the transition;
- upstream PR URL or fork branch, when available;
- blocker or exact unblock command for `parked-followup`;
- stale-check command for `needs-refresh`;
- what remains outside the current PR aperture.

Ledger notes are workflow state, not proof or policy readiness.

### `bun-ub-review-map`

Audience: reviewer deciding what to inspect first.

Required fields:

- changed files and changed unsafe/native seams;
- candidate IDs or ReviewCard IDs mapped to each seam;
- cross-language oracle map, including `oracle_language`, `oracle_path`,
  `oracle_kind`, and confidence/limitation;
- selected review comments and not-selected reasons, when `comment-plan.json`
  is present;
- repair queue bucket or manual-repair sidecar entry, when present;
- explicit no-posting boundary.

### `bun-ub-next-pick`

Audience: lane coordinator.

Required fields:

- ranked next candidate or seed;
- owner lane;
- proof mode;
- required witness, model, helper check, or fixture;
- smallest first PR;
- dependencies or parked-followup unblock;
- non-goals for the next implementer.

The ranking must be explainable from existing candidate packet, seed ledger,
proof-mode, fixture/control, and receipt state. It must not invent confidence or
claim calibrated recall.

## Machine Input

The preset renderer should accept a JSON bundle that can be composed from:

- `tokmd-packets.json`;
- `manual-candidates.json`;
- `manual-repair-queue.json`;
- `cards.json`;
- `witness-plan.md` or future witness-plan JSON;
- `receipt-audit.md` or future receipt-audit JSON;
- `repair-queue.json`;
- `comment-plan.json`;
- `docs/dogfood/stable-byte-follow-up-seeds.md` or a future seed JSON export.

The current `tokmd-packets.json` sidecar records which inputs were absent for
the manual-candidate packet export. Packet-local `stable_byte.ledger_state`
metadata and optional `oracle_map` cross-language oracle metadata are preserved
when supplied by a manual candidate and should not be reported as missing
external seed-ledger data or rendered proof. The future renderer should preserve
those limitations and add any renderer-specific absent-input notes. Missing
inputs must produce an explicit limitation, not an empty result or all-clear
statement.

## Trust Boundary

These presets are formatting contracts only. They do not run witnesses, execute
Miri, execute Bun or Node, edit source, post comments, prove site execution,
prove UB, prove memory safety, claim UB-free or Miri-clean status, provide
calibrated precision or recall, or create a default blocking policy.
