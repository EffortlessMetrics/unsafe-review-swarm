# What we learned dogfooding unsafe-review on real Rust crates

Date: 2026-06-13
Status: narrative summary, experimental corpus evidence
Corpus manifest: [`corpus.toml`](corpus.toml)
Selected judgments: [`judgments/`](judgments/)
Validation closeout: [`../status/VALIDATION_CLOSEOUT.md`](../status/VALIDATION_CLOSEOUT.md)

## Trust boundary

Everything in this document is static unsafe contract review evidence from a
characterization pass. It is not calibrated precision or recall, not a
memory-safety proof, not UB-free status, not Miri-clean status, and not
site-execution evidence unless an exact witness receipt is attached. The corpus
is a usefulness instrument, not a benchmark.

---

## The setup

`unsafe-review` finds unsafe Rust changes that are missing a safety contract,
guard, test, or witness. It does not prove code sound. Every output surface
reflects that: the tool is advisory by default, posts nothing, blocks nothing,
and runs no witnesses unless a reviewer supplies them.

To characterize whether the tool is useful on real code — not just on the
fixture suite — we ran it against seven well-known unsafe-heavy crates:
`smallvec`, `arrayvec`, `memchr`, `hashbrown`, `bytes`, `crossbeam`, and `mio`.
The corpus grew to 32 targets: 7 capped repo snapshots (up to 50 cards each)
and 23 historical PR diffs from those repositories.

All scan outputs are saved under `target/dogfood-work/` (local and untracked).
Six targets have committed reviewer judgment files. This is a small
characterization sample, not a calibration denominator.

---

## What the corpus covers

Each crate exercises a distinct part of the operation-family registry:

| Crate | Primary families exercised |
|---|---|
| `smallvec` | raw pointer reads/writes, `Vec::set_len`, pointer arithmetic, unsafe impls |
| `arrayvec` | `MaybeUninit`, `Vec::set_len`, raw pointer writes, UTF-8 unchecked, drop/deallocation |
| `memchr` | SIMD target-feature contracts, pointer arithmetic, unchecked constructors |
| `hashbrown` | large-file scanning, `MaybeUninit`, unchecked/infallible operations, unsafe-call contracts |
| `bytes` | `Vec::from_raw_parts`, slice construction, ownership-transfer |
| `crossbeam` | unsafe Send/Sync, atomics, raw pointers, strict-provenance cfg cards |
| `mio` | unsafe function call contracts, zeroed values, socket-address layout conversions |

The selection was not random. These crates are widely used, have a long public
PR history, and their unsafe patterns span most of the operation families the
tool models. That makes them useful for characterizing detection breadth; it
does not make them a precision/recall sample.

---

## What we found

### No fundamental detection gaps across operation families

Running capped repo snapshots across all seven crates produced no surprises at
the operation-family level: every family that was expected to fire based on the
fixture suite also fired on real code. The card shapes (operation, obligation,
missing evidence, suggested route) were consistent with what the fixture suite
predicts. This is a positive signal for the fixture-to-real-world transfer
assumption, not a claim that coverage is complete.

Scan times on the capped repo targets were well under 10 seconds on a
development machine. The dominant cost on real repos is the diff-scoped parse,
not per-card overhead. Capped JSON bundles stayed in the low-hundred-kilobyte
range.

### Most cards in the judgment sample were actionable

Of the 14 card or surface judgments across the 6 committed judgment files:

- 9 were `actionable`: the card carried a specific next action (a guard to add,
  a contract to write, a witness route to follow), and a reviewer could act on
  it.
- 2 were `noise`: one was a wrong-target citation (the expected card family was
  absent on that diff), one was a card that did not improve reviewability for
  its stated purpose.
- 2 were `human-only`: the card correctly identified a hard boundary (FFI layout
  or concurrency interleaving) where the right next step is human review, not
  an automated repair.
- 1 was `uncertain`.
- 0 were `missed`: no case where the tool silently omitted an unsafe obligation
  it was expected to flag.

These are small-sample counts. They are not a precision/recall figure. They are
a usefulness signal on a manually reviewed subset.

### The two corrections were framing and target selection, not detection

Two findings from the judgment pass required corrections, and neither was a
detection defect.

**Correction 1 — wrong-target citation.** An `arrayvec` target was cited as
validation evidence for the `str_from_utf8_unchecked` family. When reviewed,
the target produced no card in that family (the PR diff did not contain that
pattern). The correction was updating the judgment: the target remains useful
for the families it does exercise (unsafe function call contracts, raw pointer,
pointer arithmetic, `Vec::set_len`), but citing it as UTF-8-unchecked dogfood
was mislabeled. The tool did not misbehave; the evidence selection was wrong.

**Correction 2 — framing inconsistency.** A `requires_loom` card was reported
`agent_lsp_readiness = "ready"` while the comment-plan correctly said
`requires_witness_receipt`. Two surfaces projecting from the same ReviewCard
disagreed. The fix was adding a `RequiresWitnessReceipt` readiness state at
the single derivation point so both surfaces agree; the usefulness-telemetry no
longer over-counts `ready` cards. This was a truthfulness issue — not a
detection issue — and finding it was a concrete return on the validation pass.

### "Noise feel" is often disagreement with correct strictness

One of the more useful insights from reviewing real cards: what initially reads
as noise is often a card that is technically correct but enforces a standard
the reviewer had not internalized.

The clearest example: a `SAFETY` comment does not discharge an obligation. The
tool issues a card even when a `# Safety` comment exists, because the comment
is a contract statement, not a guard. A reviewer who expects the comment to
satisfy the card will call it noise. A reviewer who understands that
`guarded_unwitnessed` (contract present, no guard yet) is a distinct and
still-actionable class will call it a correct card with a clear next step: add
the guard or import a witness receipt.

The same pattern appears with: `align_of` used as an alignment guard (it
describes the type's alignment requirement but does not prove the pointer
satisfies it), and test mentions without receipts (a test name in a comment is
not site-execution evidence).

This is the "rigor is the product" tension: a low-noise instrument and a
rigorous one can feel like opposites to reviewers accustomed to weaker norms.
The tool's job is to be honest about what evidence is present, not to validate
existing practice.

### Evidence improvement registers as reclassification, not resolution

For an unsafe site that stays in diff scope, adding a `# Safety` contract does
not resolve the card. It reclassifies it from `contract_missing` to
`guarded_unwitnessed`. The card persists, now with a less-severe class and a
different next step (add a guard or import a witness receipt, rather than add a
contract). Resolution of a card arises when the unsafe site leaves diff scope:
for example, a doc-only change that does not touch the unsafe body, so the site
falls out of the changed hunk and the baselined card goes unmatched.

This is defensible: an unwitnessed-but-contracted unsafe site is still worth an
advisory card. It does mean "resolved" is a narrower signal than it first
appears, and "improved" (reclassification to a less-severe class) is the more
common movement for an in-scope site. See
[`../status/VALIDATION_CLOSEOUT.md`](../status/VALIDATION_CLOSEOUT.md) for the
full finding.

### Inherited debt is visible but quiet

The brownfield behavior (fixture: `raw_pointer_deref_brownfield_inherited`, and
related corpus targets with existing unsafe gaps) demonstrates the property
mature repos need for incremental adoption: a safe-only PR on a repo with pre-
existing unsafe gaps shows `new_gaps=0, inherited_gaps=N`, card class
`inherited`, and `comment_plan_status=not_eligible`. The inherited gaps are
visible in the review kit but do not generate PR comment noise.

---

## What remains open

- **Real external-repo PR noise reading.** The above characterization uses
  local fixtures and controlled historical PR diffs. Running against a live
  external PR would add signal on real-world noise. That requires network
  seeding and was deferred.
- **UTF-8 unchecked real-crate backing.** The `str_from_utf8_unchecked` family
  is fixture-backed but not yet backed by a real-crate dogfood target that
  exercises it.
- **Concurrency witness receipts.** Crossbeam and atomic-pointer cards correctly
  route to Loom/Shuttle or human review, but no saved witness receipts exist
  for those routes yet.

---

## How to read this alongside the other dogfood records

This document is a narrative summary. For structured per-target evidence see:

- [`usefulness-notes.md`](usefulness-notes.md) — per-repo and per-target
  usefulness signal, remaining noise, fixture backing, and support-tier impact
- [`index.md`](index.md) — corpus summary, judgment table, repository coverage,
  and recorded outcome movement
- [`judgments/`](judgments/) — committed reviewer judgment files (six targets)
- [`../status/VALIDATION_CLOSEOUT.md`](../status/VALIDATION_CLOSEOUT.md) —
  the first validation pass: what "fast and low-noise" is now measured by,
  fixture-sample results, the framing-consistency fix, and what remains
