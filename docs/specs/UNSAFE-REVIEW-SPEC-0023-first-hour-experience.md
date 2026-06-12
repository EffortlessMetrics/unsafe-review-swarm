# UNSAFE-REVIEW-SPEC-0023: First-hour experience

Status: accepted
Owner: product / cli
Created: 2026-05-21

Linked specs:
- [UNSAFE-REVIEW-SPEC-0011: PR and CI output](UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md)
- [UNSAFE-REVIEW-SPEC-0012: LSP and editor projection](UNSAFE-REVIEW-SPEC-0012-lsp-editor-projection.md)
- [UNSAFE-REVIEW-SPEC-0013: Agent packets](UNSAFE-REVIEW-SPEC-0013-agent-packets.md)
- [UNSAFE-REVIEW-SPEC-0019: First-run cockpit](UNSAFE-REVIEW-SPEC-0019-first-run-cockpit.md)
- [UNSAFE-REVIEW-SPEC-0024: CI design](UNSAFE-REVIEW-SPEC-0024-ci-design.md)

Linked docs:
- [docs/FIRST_USE.md](../FIRST_USE.md)
- [docs/CLI.md](../CLI.md)
- [docs/FIND_AND_FIX_UB.md](../FIND_AND_FIX_UB.md)
- [docs/editor/saved-lsp-json.md](../editor/saved-lsp-json.md)
- [docs/explanation/explain-examples.md](../explanation/explain-examples.md)
- [docs/explanation/agent-packet-examples.md](../explanation/agent-packet-examples.md)

Support-tier impact:
- [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)
- [docs/status/SUPPORT_SUMMARY.md](../status/SUPPORT_SUMMARY.md)

## 1. Purpose

The first-run cockpit covers the first successful command path. The first-hour
experience covers what happens next: a maintainer has generated a bundle, opened
the summary, explained a card, and now needs to decide whether to ask for a
guard, a contract, a targeted witness, or a bounded agent task.

The first hour should make `unsafe-review` feel like a small advisory control
panel, not a broad static analyzer dashboard.

## 2. Required user path

A successful first hour supports this path:

```bash
cargo install unsafe-review --locked
unsafe-review doctor
unsafe-review first-pr --base origin/main
open target/unsafe-review/pr-summary.md
unsafe-review explain <card-id>
unsafe-review support
```

Optional next steps:

```bash
unsafe-review context <card-id> --json
unsafe-review receipt template <card-id> --tool miri
unsafe-review receipt audit --base origin/main --format markdown --out target/unsafe-review/receipt-audit.md
unsafe-review outcome --before before.json --after after.json
```

Optional commands may be unavailable in a given release. When unavailable, docs
must name them as planned or future surfaces rather than current promises.

## 3. Behavior contract

The user should be able to answer:

```text
what unsafe seam changed
what obligation matters
what evidence exists
what evidence is missing
what to ask the author to add
which witness route is worth running
what unsafe-review is not claiming
```

The public repair loop is:

```text
first-pr -> pr-summary -> explain -> context -> witness-plan -> receipt audit -> outcome
```

Every first-hour surface must project from `ReviewCard`. No first-hour surface
may reclassify findings independently or invent a second analyzer truth.

## 4. Non-goals

- no default witness execution
- no automatic comments
- no source edits
- no default blocking policy
- no live editor requirement
- no VS Code/Open VSX publication requirement
- no safety, UB-free, Miri-clean, site-execution, or calibrated precision/recall claim

## 5. Surface expectations

`doctor`:

- reports environment readiness without failing on missing witness tools
- keeps witness tools informational by default
- states the advisory trust boundary

`first-pr`:

- writes the standard advisory bundle
- prints the summary path and top-card `explain` command
- reports no-card states honestly

`pr-summary.md`:

- acts as the reviewer front panel
- names actionable cards and next evidence requests
- frames the top card as a hypothesis, names the build/run-this-first cue, and
  includes a minimal repro cue whose limitation states unsafe-review did not run
  it or observe runtime behavior
- repeats the build/run-this-first and minimal repro cues for each card in the
  witness plan so every finding has a concrete first confirmation recipe
- preserves no-proof/no-UB-free/no-Miri-clean wording

`explain`:

- uses reviewer-first sections
- names required conditions, evidence found, evidence missing, what resolves the
  card, what does not resolve it, witness route, and trust boundary
- lists concrete related-test reach hints when available while stating that they
  are not site-execution proof

`witness-plan.md`:

- routes to credible next tools
- states what each route can and cannot show
- does not claim a witness ran unless a matching receipt exists

`lsp.json` and agent packets:

- remain saved/read-only projections
- keep action payloads command-only
- include bounded context and do-not-do rules

## 6. CI proof

```bash
cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --out-dir target/unsafe-review-first-pr-smoke

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-first-pr-smoke

cargo run --locked -p xtask -- check-docs
cargo run --locked -p xtask -- check-pr
```

## 7. Acceptance examples

- A maintainer can inspect one top card without reading JSON.
- Missing evidence is obligation-specific, not a generic "unsafe issue" label.
- No-card output says no changed unsafe-review gaps were found and preserves the
  no-proof limitation.
- A bounded agent packet has a card-scoped task, allowed repairs, confirmation
  cue with minimal repro recipe, do-not-do rules, verify commands, stop
  condition, and trust boundary.
- Saved LSP data is read-only and does not imply a live editor is required.

## 8. Promotion rule

Move to accepted when the first-use guide, CLI guide, first-pr bundle verifier,
explain examples, support posture output, saved LSP walkthrough, and agent
packet examples all align with this spec.

## 9. Zero-arg convenience entry point (`unsafe-review pr`)

`unsafe-review pr` is a pure parse-time alias for `first-pr`. It maps to the
same `FirstPr` command and produces the same advisory output bundle. No new
output surface or second analyzer truth is introduced.

### 9.1 Auto-detection behavior

When `pr` is invoked without explicit `--root`, `--base`, or `--diff` flags,
execute auto-detects the missing values before running the review:

- **root** — resolved from the git repository containing the current working
  directory (`git rev-parse --show-toplevel`).
- **base** — resolved in order: (1) `git symbolic-ref refs/remotes/origin/HEAD`
  stripped of `refs/remotes/`; (2) `git rev-parse --verify origin/main`; (3)
  `git rev-parse --verify origin/master`. The first that succeeds is used.
- **out-dir** — uses the same default as `first-pr`: `target/unsafe-review`.

When any explicit flag is supplied (`--root`, `--base`, or `--diff`),
auto-detection is skipped for all three values; the behavior is identical to
`first-pr` with those flags.

### 9.2 Unresolved-base error

If auto-detection cannot resolve a base ref (no `origin/HEAD`, no `origin/main`,
no `origin/master`), the command exits with a tool-error (exit code 2) and
prints an actionable message naming the exact flag to use:

```text
could not detect a default base ref (tried origin/HEAD, origin/main, origin/master).
Run `unsafe-review pr --base <ref>` or `unsafe-review first-pr --base <ref>`
and pass a branch, tag, or commit SHA that exists in the repository
(e.g. --base origin/main).
```

If the current directory is not inside a git repository, the error names the
explicit flags too:

```text
could not detect a git repository in the current directory (<git stderr>).
Run `unsafe-review first-pr --root <repo> --base <ref>` to supply them explicitly.
```

### 9.3 Advisory posture

`unsafe-review pr` inherits all advisory constraints of `first-pr`:

- output is advisory only; not memory-safety proof, not UB-free status, not
  Miri-clean status, and not a site-execution claim unless a matching witness
  receipt says so
- does not post comments, edit source, execute witnesses, or enforce a blocking
  policy by default

### 9.4 Help hint

The top-level help output includes a one-line hint pointing users at
`unsafe-review pr`:

```text
  pr      zero-config entry point: auto-detects root and base ref; alias for first-pr
```

`unsafe-review pr --help` routes to the `first-pr` subcommand help page.
