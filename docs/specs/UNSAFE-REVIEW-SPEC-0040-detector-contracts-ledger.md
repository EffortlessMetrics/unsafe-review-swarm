# UNSAFE-REVIEW-SPEC-0040: detector-contracts ledger

Status: accepted
Owner: core / analysis
Created: 2026-06-15

## Purpose

This spec defines the schema for `policy/detector-contracts.toml` — the
detector-discipline contract ledger introduced by the detector-discipline
control-plane lane. The ledger records, for each registered operation family,
which D1-D5 discipline obligations the detector must enforce, which fixtures
exercise it positively and negatively, which output surfaces project its cards,
and any tracked exceptions. The enforcing xtask gate
(`check-detector-contracts`) validates ledger shape and discipline declarations
against these requirements.

This spec does not introduce new analyzer behavior, new operation families, or
new output surfaces. It defines a process-discipline ledger and the schema for
its validating gate.

## Canonical source for D1–D5

The discipline checks (D1–D5) and the failure-mode taxonomy (FM1–FM9) are
defined in the SPEC-0005 appendix:

> `docs/specs/appendices/UNSAFE-REVIEW-SPEC-0005-appendix-operation-family-registry.md`

That appendix is the **canonical** and normative source for obligation
definitions. This spec references it and does not redefine, expand, or modify
the obligation set.

An earlier draft of the control-plane work proposed a 9-obligation schema; that
idea is **explicitly rejected**. The D1–D5 set from the SPEC-0005 appendix is
the correct and complete obligation set for detector discipline declarations.
No ledger entry may declare an obligation outside this set.

## Obligation identifiers

For use in `obligations` arrays, the five canonical obligation identifiers are:

| Identifier | Discipline check | Failure mode defended |
|---|---|---|
| `D1` | Unsafe-scope gate | FM1 (unsafe-scope) |
| `D2` | Definition-vs-call gate | FM2 (definition-vs-call) |
| `D3` | Same-receiver/origin discipline | FM3 (same-receiver/origin) |
| `D4` | String/comment masking | FM5 (comment/string half) |
| `D5` | Word/segment-anchored path matching | FM5 (path half) |

Not every discipline applies to every family. For example, D1 does not apply
to `unsafe_impl_send_sync`, which is detected at the item level. A contract
entry must declare only the applicable obligations; the `[[exception]]` shape
handles documented inapplicability with rationale.

## Ledger file: `policy/detector-contracts.toml`

The ledger lives at `policy/detector-contracts.toml`. It is a TOML file with a
top-level header and zero or more `[[contract]]` entries, plus zero or more
`[[exception]]` entries.

### Top-level header

```toml
schema_version = "1.0"
policy = "detector-contracts"
owner = "core / analysis"
status = "empty" | "partial" | "complete"
```

- `schema_version`: locked to `"1.0"` for this spec.
- `policy`: locked to `"detector-contracts"`.
- `owner`: the owning team.
- `status`: `"empty"` when no entries are registered (the current scaffold
  state); `"partial"` when some families are registered; `"complete"` when all
  promoted families have entries.

### `[[contract]]` entry shape

Each registered operation family gets one `[[contract]]` block:

```toml
[[contract]]
operation_family = "<string>"
obligations = ["D1", "D2", "D3", "D4", "D5"]   # subset
positive_fixtures = ["<fixture_name>", ...]
negative_fixtures = ["<fixture_name>", ...]      # REQUIRED; see below
surfaces = ["<surface_name>", ...]
evidence = "<typed note>"
review_after = "<ISO-8601 date>"
```

**`operation_family`** (string, required): must match a variant returned by
`OperationFamily::as_str()` in `crates/unsafe-review-core/src/domain/operation.rs`.
There are 36 operation families (35 named + `Unknown`). The gate rejects any
entry whose `operation_family` value does not match a registered variant.

**`obligations`** (non-empty array of obligation identifiers, required): a
subset drawn exclusively from `["D1", "D2", "D3", "D4", "D5"]` as defined in
the SPEC-0005 appendix. No other identifiers are valid. The enforcing gate
rejects an empty array or non-string entries.

**`positive_fixtures`** (non-empty array of strings, required): fixture
directory names (under `fixtures/`) that exercise a positive detection for this
family. The enforcing gate rejects an empty array or non-string entries; fixture
existence cross-checking remains a future extension.

**`negative_fixtures`** (array of strings, required): fixture directory names
(under `fixtures/`) that exercise negative controls — cases where the detector
must NOT fire. **A contract entry with an empty `negative_fixtures` array will
fail the future enforcement gate.** This is the primary invariant the ledger
enforces: every registered family must have at least one documented negative
control before its contract can be considered complete. Negative controls
should cover each applicable discipline (D1 safe-context, D2 definition header,
D3 unrelated-origin, D4 comment/string masking, D5 path segment). The fixture
suite alone is blind to assumptions the author did not know they were making;
adversarial negative controls are required before promotion.

**`surfaces`** (non-empty array of strings, required): the output surface names that
project cards from this family. Valid values are the surface names used in the
pipeline: `"json"`, `"sarif"`, `"markdown"`, `"lsp"`, `"agent"`,
`"comment_plan"`, `"witness_plan"`, `"badges"`, `"baselines"`, `"outcome"`.
The enforcing gate rejects an empty array or non-string entries.

**`evidence`** (string, required): a typed note describing the evidence type
and its relationship to the obligation. May reference fixture names or spec
sections. Not machine-validated in phase 1; present for human review.

**`review_after`** (ISO-8601 date string, required): the date after which the
contract entry should be reviewed for staleness. Format: `YYYY-MM-DD`.

### `[[exception]]` entry shape

When an obligation is documented as inapplicable to a specific family, or when
a negative control gap is tracked but not yet filled, an exception is registered:

```toml
[[exception]]
id = "<unique-string>"
rationale = "<explanation>"
owner = "<team-or-individual>"
review_after = "<ISO-8601 date>"
```

**`id`** (string, required): a stable unique identifier for this exception,
used for cross-referencing in reviews and issue comments.

**`rationale`** (string, required): explains why the obligation is inapplicable
or why the gap is temporarily allowed. Must name the affected obligation(s) and
family. Must not assert soundness, UB-freedom, or Miri-clean status.

**`owner`** (string, required): the person or team responsible for reviewing
and resolving this exception.

**`review_after`** (ISO-8601 date string, required): the date after which this
exception must be re-evaluated. Expired exceptions without renewal will fail
the future enforcement gate once that gate is active.

## Gate: `check-detector-contracts`

The `check-detector-contracts` xtask gate validates the ledger. It is wired
into `check-pr` and is now **enforcing** — structural violations and
undocumented gaps fail the gate.

### What the gate checks

**Blocking findings (always fail):**

- Malformed TOML or missing `schema_version`.
- Missing or empty `operation_family` in a `[[contract]]` entry.
- Duplicate `operation_family` values.
- Missing or empty `obligations` array, or non-string/empty-string entries.
- Missing or empty `positive_fixtures` array, or non-string/empty-string entries.
- Empty `negative_fixtures` array (or absent) without a documented gap.
- Non-string/empty-string `negative_fixtures` entries.
- Missing or empty `surfaces` array, or non-string/empty-string entries.
- Missing or empty `id` in a `[[exception]]` entry.
- Duplicate `id` values in `[[exception]]` entries.

**Documented-gap path (tracked exceptions, not blocking):**

A contract that lacks negative controls may acknowledge the gap by adding
three optional fields to the `[[contract]]` entry:

```toml
proof_gap  = "<explanation of the gap>"
owner      = "<team or individual responsible>"
review_after = "<ISO-8601 date>"
```

All three must be non-empty. When present, the gate prints the finding as a
tracked exception (suffix: `(tracked exception)`) and does not fail. If any
of the three fields is missing or empty, the gap is blocking.

This documented-gap path is distinct from the whole-contract `[[exception]]`
opt-out. Use `[[exception]]` for an obligation that is structurally
inapplicable to a family. Use `proof_gap + owner + review_after` on a
`[[contract]]` entry when negative controls are temporarily absent.

**Tracked exceptions do not change the advisory posture of findings.** They
record that a discipline gap is acknowledged and owned, not that the detector
is correct or complete.

The gate validates **process discipline and ledger shape only**. It does not
validate that the detector is correct, that it finds all unsafe operations, or
that its evidence discharge logic is sound.

## Trust boundary

unsafe-review is an **advisory** static-review tool. This spec and the
detector-contracts ledger preserve that boundary without exception:

- The detector-contracts ledger does not **prove** any detector is correct,
  complete, or free of false positives.
- The ledger does not claim **UB-free** or **Miri-clean** status for any
  operation family or detection site.
- The ledger does not constitute **site execution** evidence or witness
  execution results. Witness receipts remain a separate system (see SPEC-0009).
- The gate (`check-detector-contracts`) validates ledger **shape** and
  **discipline declarations** — it never validates soundness, memory-safety
  proof, or calibrated precision or recall.
- No ledger entry or gate result changes the advisory posture of unsafe-review
  output. Findings remain advisory; no surface **blocks** merges or posts
  comments by default as a result of ledger registration.
- Syntax-first analysis remains the default. The ledger records which D1–D5
  checks a detector applies; it does not mandate type-aware, MIR-based, or
  build-required analysis paths.

The ReviewCard is the single truth object. All output surfaces — JSON, SARIF,
markdown, LSP diagnostics, agent packets, comment plan, witness plan, badges,
baselines — project from the same card. The detector-contracts ledger records
which discipline obligations constrain card emission for each family; it does
not introduce a second truth surface.

## Current ledger state

`policy/detector-contracts.toml` carries 11 registered families as of the
control-plane PR-C enforcement flip. All 11 entries have non-empty
`negative_fixtures` arrays, so the gate passes clean on current main. The
gate is now enforcing: any future contract entry with an empty
`negative_fixtures` must carry `proof_gap + owner + review_after` or the gate
fails.

## Implementation tracking

- SPEC-0041: documents the dispatch architecture this ledger complements.
  Status: accepted.
- SPEC-0040 (this spec): defines the ledger schema. Status: accepted.
- PR-3 (control-plane lane): `policy/stance-decisions.toml`. Status: merged.
- PR-5 (control-plane lane): xtask gates born informational. Status: merged.
- PR-6 (control-plane lane): high-risk families registered. Status: merged.
- PR-C (control-plane lane): enforcement flip — gates now block on structural
  violations and undocumented gaps. Status: merged (this PR).

See `.rails/lanes/control-plane/implementation-plan.md` for the full sequence.
