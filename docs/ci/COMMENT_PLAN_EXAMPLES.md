# Comment-plan examples

`comment-plan.json` is the quiet inline-comment surface for PR review. It is a
plan artifact only: `unsafe-review` does not post comments, run witnesses, edit
source, or make a blocking policy decision.

These examples are fixture-backed. Regenerate them with:

```bash
for fixture in \
  raw_pointer_alignment \
  copy_nonoverlapping \
  public_unsafe_fn_missing_safety \
  ffi_sanitizer_route \
  safe_code_no_cards
do
  cargo run --locked -p unsafe-review -- first-pr \
    --root "fixtures/$fixture" \
    --diff "fixtures/$fixture/change.diff" \
    --out-dir "target/unsafe-review-comment-plan-$fixture"
done
```

Then verify each bundle:

```bash
for fixture in \
  raw_pointer_alignment \
  copy_nonoverlapping \
  public_unsafe_fn_missing_safety \
  ffi_sanitizer_route \
  safe_code_no_cards
do
  cargo run --locked -p xtask -- check-first-pr-artifacts \
    "target/unsafe-review-comment-plan-$fixture"
done
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
  "summary": {
    "selected_count": 1,
    "not_selected_count": 0,
    "budget": 3,
    "reason": "bounded reviewer noise"
  },
  "comments": [
    {
      "class": "guard_missing",
      "operation_family": "raw_pointer_read",
      "changed_line": true,
      "actionability": "specific_guard_missing",
      "relevance": "medium",
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

## Selected candidate — copy range guard missing

Fixture: `copy_nonoverlapping`

The same `guard_missing` shape recurs for `core::ptr::copy_nonoverlapping`
when length, overlap, alignment, or initialization guards are not visible at
the call site. The plan keeps the comment specific:

```json
{
  "comments": [
    {
      "class": "guard_missing",
      "operation_family": "copy_nonoverlapping",
      "actionability": "specific_guard_missing",
      "relevance": "medium",
      "selection_reason": "actionable high-priority review card",
      "next_action": "Add or expose the local guard that discharges the `copy_nonoverlapping` safety obligation."
    }
  ]
}
```

The candidate body asks for the missing length / overlap guard explicitly; it
does not say "copy is unsafe" or "this is UB".

## Family-budget suppression

The inline plan is capped by count and by operation family. If multiple
high-signal cards share one `operation_family`, the first selected card keeps
the inline comment slot and the others stay visible in `not_selected[]`:

```json
{
  "not_selected": [
    {
      "operation_family": "raw_pointer_read",
      "changed_line": true,
      "reason": "operation family already selected for comment-plan budget"
    }
  ]
}
```

This keeps future comment posting quiet without hiding the underlying
ReviewCards from `cards.json`, `pr-summary.md`, SARIF, witness plan, saved
`lsp.json`, or agent context.

Cards without a changed-line anchor also stay out of `comments[]` and remain
visible in `not_selected[]` with reason `outside changed hunk`. That preserves
the inline review budget without turning unchanged repository inventory into
future PR comments.

## Card present, not selected — public unsafe fn missing `# Safety`

Fixture: `public_unsafe_fn_missing_safety`

When a changed `pub unsafe fn` lacks a precise public `# Safety` section,
`unsafe-review` still emits a `contract_missing` card. The inline comment plan
keeps it out of `comments[]` because the operation family is `unknown`, which
is often an owner-contract or inventory-like surface rather than a precise
changed unsafe operation. The card remains visible in the bundle.

```json
{
  "comments": [],
  "not_selected": [
    {
      "class": "contract_missing",
      "operation": "pub unsafe fn caller_must_uphold_contract() {",
      "operation_family": "unknown",
      "changed_line": true,
      "next_action": "Add a precise public `# Safety` section that names the required caller obligations.",
      "actionability": "specific_contract_missing",
      "relevance": "high",
      "reason": "operation family unknown"
    }
  ]
}
```

`operation_family` is `unknown` because the unsafe contract obligation lives
on the `unsafe fn` declaration itself, not on a single unsafe operation. The
bundle still asks for explicit caller obligations rather than for safety prose.

## Card present, not selected — human-review-only FFI

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
      "operation": "unsafe extern \"C\" {",
      "operation_family": "ffi",
      "changed_line": true,
      "next_action": "Use sanitizer/cargo-careful or an explicit FFI boundary contract; Miri may not exercise this seam.",
      "actionability": "specific_witness_missing",
      "relevance": "low",
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
  "summary": {
    "selected_count": 0,
    "not_selected_count": 0,
    "budget": 3,
    "reason": "bounded reviewer noise"
  },
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
- `summary.selected_count`, `summary.not_selected_count`, and
  `summary.budget` must match the bounded review budget;
- one planned comment per `card_id`;
- one planned comment per `path`/`line`;
- `changed_line = true`, renderable locations only;
- no `static_unknown`, baseline-known, or suppressed planned comments;
- no `operation_family: "unknown"` planned comments;
- every planned body stays at or below 220 words;
- every ReviewCard appears in either `comments[]` or `not_selected[]`;
- `not_selected` entries must reference known cards and cannot repeat planned
  comment cards;
- every selected and not-selected entry carries `actionability` and
  `relevance` (`high` / `medium` / `low`);
- trust-boundary text remains present.
