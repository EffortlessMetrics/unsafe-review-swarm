# Saved LSP JSON workflow

This walkthrough is for maintainers who want to inspect the current
editor-adjacent surface without installing an editor extension or starting a
live LSP server.

The supported surface today is a saved JSON projection:

```text
target/unsafe-review/lsp.json
```

It is read-only. It projects existing `ReviewCard`s into editor-shaped
diagnostics, hovers, and command payloads. It must not create analyzer truth
outside the cards.

## Generate the projection

For a normal PR review, run:

```bash
unsafe-review first-pr --base origin/main
```

That writes the full advisory bundle, including:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/lsp.json
```

For only the saved editor projection, run:

```bash
unsafe-review check \
  --base origin/main \
  --format lsp \
  --out target/unsafe-review/lsp.json
```

For a deterministic fixture smoke from a repo checkout:

```bash
unsafe-review check \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --format lsp \
  --out target/unsafe-review/lsp.json
```

## Inspect what an editor would show

Open `target/unsafe-review/lsp.json` and look for:

- `status`: advisory mode and trust-boundary text,
- `diagnostics`: one entry per ReviewCard site,
- `hovers`: compact card explanations with required conditions, evidence
  summaries, missing evidence, next action, witness route, and trust boundary,
- `code_actions`: command-shaped copy/open actions with stable object
  `payload` fields and legacy positional `arguments`.

The projection is useful when checking whether a card would be explainable in
an editor before any live client exists.

## Use the card ID

Every diagnostic carries a `card_id`. Use that ID with the normal CLI surfaces:

```bash
unsafe-review explain <card-id>
unsafe-review context <card-id> --json
```

`explain` is the reviewer view. `context` is the bounded repair-packet view for
LLMs or agents. Both still read from ReviewCards; neither edits source or runs
witnesses.

## Current limits

This saved projection is not:

- a live LSP server,
- a VS Code or Open VSX extension,
- a source-editing quick fix,
- a witness runner,
- a policy gate,
- a safety, UB-free, Miri-clean, or site-execution claim.

No diagnostics means no saved ReviewCards for that scope. It does not prove the
repository safe, UB-free, Miri-clean, or that any unsafe site executed.

## Future adapter contract

A future editor adapter should consume this same artifact shape first:

- show diagnostics from `diagnostics`,
- show hover text from `hovers`,
- copy bounded agent packets through existing `context` data,
- copy witness commands from existing witness routes,
- open related tests when static reach evidence exists.

The adapter must stay read-only in v0.x. It must not apply patches, insert
SAFETY comments, run Miri, run sanitizers, create receipts, or post PR
comments.
