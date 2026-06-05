# Dogfood Reviewer Judgment Schema

Status: experimental measurement input schema

Reviewer judgments live at:

```text
docs/dogfood/judgments/<target>.toml
```

They record whether a selected dogfood review kit helped a maintainer, author,
or agent take the next review action. A judgment is a manual usefulness sample,
not a calibration report, precision/recall denominator, support-tier promotion,
policy decision, witness result, or safety claim.

## File Shape

```toml
schema_version = "1.0"
target = "arrayvec-pr288"
report = "reports/2026-05-28-arrayvec-first-pr-projection-smoke.md"
reviewer = "manual"
date = "2026-05-31"
scope = "first-pr review packet"
artifact_set = "target/dogfood-work/arrayvec-pr288.first-pr-smoke"
cards_artifact = "fixtures/vec_set_len_self_new_const_cap_not_guard/expected.cards.json"
trust_boundary = "Static unsafe contract review measurement input; not calibrated, not a proof of memory safety, not UB-free status, not a Miri result, not site execution evidence, not witness adequacy, and not policy readiness."

[[cards]]
card_id = "UR-arrayvec-src-array-string-rs-from-byte-string-operation-vec_set_len-set-len-073a0fa631f6-initialized_memory-c1"
family = "vec_set_len"
judgment = "actionable"
reason = "The top card names the initialized-memory obligation and asks for a witness receipt instead of implying proof."
next_step = "Keep as a reviewer action sample; do not promote to calibration without judged denominator data."

[[missed]]
file = "src/lib.rs"
line = 123
expected_family = "ffi"
status = "open"
reason = "Manual review found a changed unsafe boundary without a useful ReviewCard."
next_step = "Create a follow-up seed before changing analyzer behavior."
```

`artifact_set` is optional when the report records the artifact location
directly. `cards_artifact` is optional and should name a checked-in JSON card
snapshot when card IDs need mechanical validation; local `target/` artifacts
remain report evidence, not CI-readable source-of-truth files. `card_id` is
optional unless `cards_artifact` is present. Missed-card observations do not
have a ReviewCard yet.

[`index.json`](../index.json) publishes the selected real-crate judgment sample
counts derived from committed judgment files. Those counts are a repeatability
denominator for manual usefulness rows only; they are not an accuracy
denominator, calibration report, precision/recall claim, witness result, or
policy gate.

## Checked References

`cargo run --locked -p xtask -- check-dogfood` verifies committed judgment
files:

- `target` exists in the dogfood corpus manifest.
- `report` links under `reports/` and the report file exists.
- `family` and `expected_family` are known operation families or review-kit
  surfaces.
- `judgment` and missed-card `status` use the closed vocabularies below.
- `trust_boundary` preserves static-review, memory-safety, not UB-free status,
  not-a-Miri-result, site-execution, witness-adequacy, precision/recall, and
  policy limits.
- if `cards_artifact` is present, every card judgment must name a `card_id`
  that exists in that checked-in snapshot.
- `index.json` and [`index.md`](../index.md) match the committed real-crate
  judgment-file target, row, and label counts.

## Judgment Labels

Use one label per card judgment:

| Label | Use when | Do not infer |
|---|---|---|
| `actionable` | The card helped the reviewer ask for concrete evidence or a bounded next step. | The code is unsafe, proven, or policy-blocking. |
| `noise` | The card was broad, duplicated, stale, wrong-target, poorly ranked, or not useful for the PR. | The entire family is unsupported. |
| `missed` | Manual review found an obligation that should become a follow-up seed. | Global recall or analyzer failure rate. |
| `uncertain` | The reviewer could not decide usefulness without more context. | The card is either correct or incorrect. |
| `human-only` | The next useful action is human deep review rather than a bounded repair. | Witnesses or agents cannot ever add signal. |
| `good-agent-task` | The packet gives an agent a bounded, reviewable task. | Automatic repair or source editing by default. |
| `bad-agent-task` | The packet is too broad, underconstrained, or likely to cause unrelated edits. | The ReviewCard itself is invalid. |

## Required Questions

Each judgment file should answer the relevant subset:

- Was the top card useful?
- Was the missing or weak evidence clear?
- Was the next action concrete?
- Was the witness route understandable?
- Was the comment plan too noisy?
- Was the repair queue bucket sane?
- Would `context <card-id> --json` be a good agent task?
- Did any card overclaim?
- Did manual review find a missed obligation?

## Trust Boundary

Dogfood reviewer judgments are static unsafe contract review measurement inputs.
They are not calibrated precision or recall, not a proof of memory safety, not
UB-free status, not a Miri result, not Miri-clean status, not site execution
evidence, not witness adequacy, not release readiness, and not policy readiness.
They may support dogfood-observed wording only after a later checked claim names
the target, report, judgment file, and known limits.
