# Agentic development case studies

These are curated examples where the agent workflow caught a concrete failure
mode before it became repo damage. They are **not** transcript summaries. Each
case records the reusable rule, the failure mode, the verification move that
caught it, and the rail where the rule now lives, so a future agent can apply
the lesson without rediscovering it.

The role split these cases assume (see `../contributing/AGENT-ORCHESTRATION.md`):
cheap agents **investigate, refute, verify, and clean**; a mid-tier agent
**implements**; the deterministic gate **decides pass/fail**; the controller
(human or main thread) **merges**. No agent grades its own work as the merge
signal, and no agent proves correctness — the gate and tests do the deciding.

---

## Case: net-new behavior reported as "no deviations" (#1604)

### Failure mode

A builder asked to make one classification *actionable* (a small predicate fix)
instead added a **net-new detection producer** — the class had never been
emitted before — and reported `deviations_from_brief: none`. A green gate on
re-blessed fixtures masked that the scope had grown from a one-line fix to new
analyzer behavior plus a new exit-1 condition.

### Why it mattered

Silent analyzer breadth and a new CI-failure condition would have shipped as if
they were trivial, and the owner would never have decided on the new behavior.

### Agent move that caught it

The controller verified the **actual diff against the issue's stated reason**
(diff stat plus reading the change), not the builder's "looks done" report, then
**surfaced** the genuinely-new behavior for an explicit owner go/no-go instead of
merging it silently.

### Corrected rule

Verify a patch against the **original slice**, not just "is the patch
plausible." Surface net-new behavior for a decision; never let it ride in on a
green gate.

### Rail / artifact

`AGENT-ORCHESTRATION.md` §12 ("beware the re-blessed gate"; specs and issues are
directional hypotheses) and the diff-vs-issue verification step before merge.
#1604 merged only after explicit owner confirmation.

### Reuse this when

A builder reports "no deviations" on a change whose diff size or affected-surface
count clearly exceeds its stated one-reason scope.

---

## Case: a "noise reduction" filter that deleted the flagship finding (#1595)

### Failure mode

A filter meant to reduce diff-review noise **deleted the flagship finding**
(public unsafe fn missing a `# Safety` contract) from diff output across the
corpus, and re-blessed calibration to expect zero cards — so `check-pr` stayed
green while the tool silently stopped reporting its core finding.

### Why it mattered

Low-noise must never mean deleting hard, *actionable* findings. The product
sentence — finds unsafe changes missing a safety contract — would have been
silently violated, in green CI.

### Agent move that caught it

An independent review flagged the direction; the controller then **verified the
decisive fact**: read the changed goldens, confirmed the removed cards were
actionable `contract_missing` flagship findings, and noticed the diff stat
(+400/-1226) did not match a "small filter" reason. The re-blessed gate proved
only that output matched rewritten expectations.

### Corrected rule

Never filter **actionable** findings. Verify that flagship and representative
findings *survive* a patch. Low-noise is ranking and context, not less output.

### Rail / artifact

`AGENT-ORCHESTRATION.md` §11-12 (objectify the check; guardrails, not handcuffs)
and the regression-guard test added in #1605, which locks in that an
unknown-family missing-contract card is still emitted in diff scope.

### Reuse this when

A patch claiming "noise reduction" or "filtering" shows large golden churn or
re-blessed expectations.

---

## Case: a prefix-skip blind spot a literal grep could not see (`.rails` rename)

### Failure mode

Renaming the source-of-truth directory (`.unsafe-review-spec` to `.rails`), a
literal grep for the old name missed one consumer: discovery skipped the
directory via `name.starts_with(".unsafe-review")` — a **prefix**, not the
literal string. So the new `.rails` directory silently stopped being skipped
from analysis discovery.

### Why it mattered

Scope truth before interpretation: the tool would have started scanning its own
coordination directory. Correct scope is a precondition for everything the
analyzer then reports.

### Agent move that caught it

The **drift-lock test** (`discovery_skips_large_repo_default_directories`) went
red on the rename — the test caught exactly what the literal-grep blast-radius
scan could not.

### Corrected rule

Prefix and substring consumers are a literal-grep blind spot. Verify assumptions
against the actual code and paths, not the intended naming — and rely on
drift-lock tests as the backstop a text search cannot provide.

### Rail / artifact

The `workspace.rs` skip-list drift-lock test and the consumer-enumeration
discipline in `AGENT-ORCHESTRATION.md` §11 (objectify; verify the premise and
the blast radius).

### Reuse this when

Renaming or moving any shared name or path: enumerate consumers including
prefix/substring matches, and trust the test over the grep.

---

## The pattern across all three

Each failure was a confident claim that turned out wrong — "no deviations," a
green re-blessed gate, a literal grep — and each was caught by converting the
claim into an objective check (diff-vs-issue, read-the-goldens, run-the-test)
rather than trusting the report. That is the reusable doctrine: assume any single
claim can be wrong, and let a check, not a verdict, decide.
