# #2224 PR1 batch protocol: matching rules and baseline questions

Status: batch-scoped. These rules govern only `docs/accuracy/batches/2224-pr1/`.
They do not define global precision or recall.

## The four baseline questions

Every expected seam answers all four in `mapping.toml`:

1. **Found + operation (Q1).** Did the analyzer emit a card covering the seam's
   source anchor with the correct operation family?
2. **Obligations without over-credit (Q2).** Did the card name the seam's
   expected obligations, and did it avoid crediting evidence the source does
   not provide (wrong-shape guards, comments as discharge, removed asserts)?
3. **Useful selection (Q3).** Would a reviewer working this PR scope want this
   card surfaced? Test-harness unsafe calls with in-bounds buffers are genuine
   seams but usually not useful selections; the mapping says so explicitly
   instead of deleting them from the denominator.
4. **Next action (Q4).** Was the proposed next action correct, sufficiently
   specific to this seam, and feasible (no witness execution assumed, no
   third-party writes, no commands that cannot run in this scope)?

## Predeclared matching rules

Each emitted card gets exactly one disposition, each expected seam exactly one
outcome. The evaluator recomputes every count from these rows.

| Outcome | Meaning |
|---|---|
| `match` | Card covers the seam anchor with the right operation family. Q2–Q4 still judged independently; a match can carry `over_credited = true` or `useful = false`. |
| `missing` | Expected seam with no covering card. |
| `wrong-family` | Card covers the seam anchor but names the wrong operation family. Counts against operation accuracy, not recall. |
| `wrong-obligation` | Card covers the seam but its obligation evidence is wrong in a way that changes the review (missed live obligation, or discharge credited from non-evidence). Counts against obligation correctness. |
| `duplicate` | Second or later card covering an already-matched seam. First card keeps `match`; extras are duplicates. |
| `wrongly-surfaced` | Emitted card covering no expected seam (false alarm), including every card in the quiet-control case. |
| `infeasible-action` | Card is otherwise right but its next action cannot be carried out in scope. Recorded on the card, orthogonal to the seam outcome. |
| `unknown` | Partial or failed run leaves the seam unjudgeable. Unknowns are reported as their own count and keep denominators honest. |

## Counting rules

- Seam recall = `match / (match + missing)`. `wrong-family` counts as found
  for recall (the site was flagged) but against operation accuracy.
- Operation accuracy = `right-family matches / (match + wrong-family)`.
- Obligation over-credit = seams with `over_credited = true`, reported as a
  count and rate over matched seams, never folded into recall.
- Selection usefulness = `useful / selected`, where `selected` is the set of
  cards the mapping reviewer would surface; `not selected` cards keep their
  seam outcomes.
- Quiet-PR false alarms = all cards emitted in `arrayvec-143`, each
  `wrongly-surfaced` by rule.
- Empty denominators are `unknown`, never 100% and never 0%. The quiet control
  has no recall; the batch has recall only over the 8 non-control seams.
- Unsupported families, parse failures, excluded inputs, and incomplete runs
  get named counts in the report. They cannot be silently removed.

## Ordering rule

`seams.toml` is recorded from diff reading before analyzer output is consulted
(`analyzer_outputs_consulted = false`). Mapping never edits the seam inventory
to fit the output; disagreements become `missing` / `wrongly-surfaced` rows or
visible unresolved judgments with their effect on counts stated.
