# Comment-plan examples

`comment-plan.json` is the quiet inline-comment surface for PR review. It is a
plan artifact only: `unsafe-review` does not post comments, run witnesses, edit
source, or make a blocking policy decision.

These examples are fixture-backed. Regenerate them with:

```bash
cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --out-dir target/unsafe-review-comment-plan-selected

cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/ffi_sanitizer_route \
  --diff fixtures/ffi_sanitizer_route/change.diff \
  --out-dir target/unsafe-review-comment-plan-not-selected

cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/safe_code_no_cards \
  --diff fixtures/safe_code_no_cards/change.diff \
  --out-dir target/unsafe-review-comment-plan-no-card
```

Then verify each bundle:

```bash
cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-comment-plan-selected

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-comment-plan-not-selected

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-comment-plan-no-card
```

## Selected candidate

Fixture: `raw_pointer_alignment`

Why it is selected:

- the card is `guard_missing`;
- the priority is high;
- the card has a changed-line location;
- the next action names a specific local guard obligation;
- the witness route is copied from the `ReviewCard`.

Representative fields:

```json
{
  "comments": [
    {
      "class": "guard_missing",
      "operation_family": "raw_pointer_read",
      "actionability": "specific_guard_missing",
      "selection_reason": "actionable high-priority review card",
      "next_action": "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation.",
      "verify_commands": [
        "cargo +nightly miri test read_header",
        "cargo +nightly careful test read_header"
      ]
    }
  ]
}
```

The body remains bounded and repeats the plan boundary:

```text
Plan boundary: artifact-only inline comment candidate; unsafe-review did not
post this comment, run witnesses, or make a policy decision.
```

## Card present, not selected

Fixture: `ffi_sanitizer_route`

This fixture emits a `ReviewCard`, but the inline plan stays quiet. The card is
still visible in `cards.json`, `pr-summary.md`, `witness-plan.md`, SARIF, and
saved `lsp.json`; it is just not promoted into an inline comment candidate.

Representative fields:

```json
{
  "comments": [],
  "not_selected": [
    {
      "class": "miri_unsupported",
      "operation_family": "ffi",
      "actionability": "specific_witness_missing",
      "reason": "priority/confidence below inline comment threshold"
    }
  ]
}
```

This keeps the PR comment budget focused without hiding the advisory card.

## No changed gaps

Fixture: `safe_code_no_cards`

When no changed unsafe-review gaps are found, `comment-plan.json` has no
planned comments and carries the no-card limitation:

```json
{
  "comments": [],
  "no_changed_gaps": {
    "message": "No changed unsafe-review gaps were found.",
    "limitation": "This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed."
  }
}
```

This is not an all-clear result. It only means this first-pr run did not emit
changed unsafe-review gaps.

## Invariants

The verifier treats these as artifact contract rules:

- at most three planned comments;
- one planned comment per `card_id`;
- one planned comment per `path`/`line`;
- changed-line, renderable locations only;
- no `static_unknown`, baseline-known, or suppressed planned comments;
- every planned body stays at or below 220 words;
- `not_selected` entries must reference known cards and cannot repeat planned
  comment cards;
- trust-boundary text remains present.
