# CLI guide

`unsafe-review` is a cheap PR-time unsafe contract review tool. It emits
`ReviewCard`s and projects those same cards into human output, JSON, PR
artifacts, saved editor data, agent packets, repo posture, badges, and receipt
evidence.

Every command is advisory by default. The tool does not prove memory safety, does
not claim UB-free status, does not run witness tools by default, and does not
post PR comments.

## Support Posture

Print the current support posture and trust boundary without analyzing the repo:

```bash
unsafe-review support
```

This is the first command to run when you need to know what is experimental,
advisory, deferred, or not default. It reports that `ReviewCard`s are the source
of truth, `first-pr` artifacts are advisory projections, receipts import saved
external evidence only, policy reports are advisory, witness execution is not
default, comment posting is not default, source edits are not supported, and
live LSP remains deferred.

## Review A Diff

Review the current branch against `origin/main`:

```bash
unsafe-review check --base origin/main
```

Review a supplied unified diff:

```bash
unsafe-review check --diff change.diff --format json
git diff origin/main...HEAD | unsafe-review check --diff - --format json
```

Use `--root` when reviewing a fixture or another workspace:

```bash
unsafe-review check \
  --root fixtures/raw_pointer_alignment \
  --diff change.diff \
  --format json
```

The default policy is `advisory`; it reports cards but does not fail the
command. The explicit no-new-debt mode exits nonzero when unbaselined actionable
gaps remain:

```bash
unsafe-review check --base origin/main --policy no-new-debt
```

Blocking policy is not implemented.

Generate a non-blocking no-new-debt policy report without changing command exit
behavior:

```bash
unsafe-review policy report \
  --root . \
  --base origin/main \
  --format markdown \
  --out target/unsafe-review/policy-report.md
```

The policy report compares current `ReviewCard`s with exact baseline and
suppression ledgers. It counts new gaps, baseline-known cards, suppressed cards,
resolved baseline entries, and expired suppressions. Current-card entries also
show the operation expression, operation family, policy reason, and next action
from the same `ReviewCard`. JSON reports include schema-versioned
classification explanations, limitations, unmatched baseline entries, and
invalid-ledger-entry fields. It does not block, execute witnesses, or create
broad suppression authority.

## First PR Bundle

For a first local review pass, write the standard advisory artifact bundle:

```bash
unsafe-review first-pr --base origin/main
```

`review` is an alias for `first-pr`.

By default this writes:

```text
target/unsafe-review/review-kit.json
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/github-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/receipt-audit.md
target/unsafe-review/lsp.json
target/unsafe-review/repair-queue.json
```

Use `--out-dir <dir>` to choose another artifact directory, or `--diff file|-`
to review a supplied diff.

The command analyzes once and renders every artifact from the same
`ReviewCard`s. It stays advisory-only: it does not execute witness tools, post
comments, edit source, or enforce blocking policy.

The bundle also includes `receipt-audit.md`, and the terminal handoff prints the
matching `unsafe-review receipt audit` command so reviewers can check whether
saved witness receipt metadata still matches the current first-pr cards. The
audit is metadata-only and does not run the witness.

## Output Formats

All output formats project the same `ReviewCard`s. They must not reclassify
findings independently.

| Format | Command | Use |
|---|---|---|
| `human` | `unsafe-review check --base origin/main` | terminal review |
| `json` | `unsafe-review check --base origin/main --format json` | canonical machine-readable cards with operation, evidence, routes, and next action |
| `markdown` | `unsafe-review check --diff change.diff --format markdown` | local report with operation and next-action context |
| `pr-summary` | `unsafe-review check --base origin/main --format pr-summary --out target/unsafe-review/pr-summary.md` | sparse reviewer-facing PR artifact |
| `github-summary` | `unsafe-review check --base origin/main --format github-summary --out target/unsafe-review/github-summary.md` | bounded `GITHUB_STEP_SUMMARY` doorway that points to the full artifact bundle |
| `sarif` | `unsafe-review check --base origin/main --format sarif --out target/unsafe-review/cards.sarif` | code-scanning-compatible artifact |
| `comment-plan` | `unsafe-review check --base origin/main --format comment-plan --out target/unsafe-review/comment-plan.json` | artifact-only inline comment candidates with card ID, operation, next action, actionability, routes, and verify commands |
| `lsp` | `unsafe-review check --base origin/main --format lsp --out target/unsafe-review/lsp.json` | saved editor diagnostics and hovers |
| `witness-plan` | `unsafe-review check --base origin/main --format witness-plan --out target/unsafe-review/witness-plan.md` | reviewer-facing witness route plan |

`repair-queue.json` is currently emitted by `first-pr`. It groups ReviewCards
into copy-only guard, contract, test, witness, human-review, and
do-not-auto-repair buckets, each pointing back to
`unsafe-review context <card-id> --json`. It is not a standalone `--format`
yet, and it does not run agents.

The default human output is for terminal review. It names the card identity,
operation family, operation expression, obligation evidence, witness route, next
action, verify commands, and trust boundary without executing witnesses.

`comment-plan` is plan-only. It carries the concrete ReviewCard operation
expression, next action, actionability, routes, and verify commands for each
planned comment and does not post comments. When no changed
unsafe-review gaps are found, `comments` is empty and the artifact includes a
`no_changed_gaps` message with the same no-proof limitation used by the terminal
and Markdown surfaces.

`lsp` writes saved JSON only. It includes a read-only status object,
diagnostics, hovers, and command data for copying packets, copying witness
commands, explaining routes, and opening statically related tests. There is no
editor extension or live LSP server in this surface.

`witness-plan` is a routing artifact. It groups existing `ReviewCard`s by
witness family: Miri / `cargo-careful`, sanitizers, Loom / Shuttle, Kani /
Crux, and human deep review / unsupported. Each route entry includes why that
route fits, what it can show, what it cannot prove, a suggested command when one
is available, and a receipt import hint. It does not run those commands.

## PR Artifacts

The advisory workflow renders:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
```

Verify a local or downloaded artifact set with:

```bash
cargo xtask check-advisory-artifacts target/unsafe-review
```

That verifier checks parseability, advisory policy, plan-only comment mode,
projected card identity consistency, result counts, trust-boundary text, and
absence of positive safety/proof overclaims. It does not prove the analyzer
found every unsafe issue.

For the full `first-pr` bundle, including `witness-plan.md` and saved
`lsp.json`, use:

```bash
cargo xtask check-first-pr-artifacts target/unsafe-review
```

That verifier keeps the bundle advisory: it checks route limitations,
comment-plan caps and renderable inline fields, saved LSP diagnostic evidence
and action payloads, zero-gap wording, card identity consistency, and absence
of positive safety/proof wording. It does not run witnesses, post comments,
edit source, or make a policy decision.

## Explain And Context

Use `explain` for a human-readable explanation of one card:

```bash
unsafe-review explain --root fixtures/raw_pointer_alignment <card-id>
```

The explanation is reviewer-first: why the card exists, required safety
conditions, evidence found, evidence missing, what would resolve it, what would
not resolve it, the recommended witness route, and the static-review trust
boundary. It does not execute witnesses. See
[Explain examples](explanation/explain-examples.md) for fixture-backed examples
of common card families.

Use `context` for the bounded agent packet:

```bash
unsafe-review context --root fixtures/raw_pointer_alignment <card-id> --json
```

The context packet is copy-only. It includes a card-scoped task, missing
evidence, allowed repairs, do-not-do rules, verify commands, stop conditions,
and the static-review trust boundary. It does not execute an agent and does not
edit source. See
[Agent packet examples](explanation/agent-packet-examples.md) for
fixture-backed examples of repair-ready and human-review-only packets.

## Manual Candidates

Import a manually discovered advisory candidate:

```bash
unsafe-review candidate import target/unsafe-scout/textdecoder-candidate.json \
  --out .unsafe-review/candidates/R4R2-S001.json
```

The imported artifact is canonicalized with `source = "manual"` and
`manual_candidate = true`. It remains advisory and must not be described as an
analyzer-discovered finding.

After import, `explain` and `context` can load the candidate by ID from
`.unsafe-review/candidates/` when no analyzer ReviewCard with that ID exists:

```bash
unsafe-review explain R4R2-S001
unsafe-review context R4R2-S001
unsafe-review candidate witness-plan R4R2-S001
```

Manual candidate projections preserve the manual marker and external evidence
references. They do not execute witnesses, post comments, edit source, enforce
policy, prove UB, prove site execution, or prove repository safety.

## Repo Posture And Badges

Repo mode scans the workspace and reports static open unsafe-review gaps:

```bash
unsafe-review repo --format json
unsafe-review repo --format markdown --out target/unsafe-review/repo-posture.md
```

When `repo` writes a report with `--out`, it renders to `<out>.partial` and
renames that file to `<out>` only after a successful render. It also updates
`<out>.status.json` while analysis runs. The status sidecar records the scan
phase, elapsed time, discovered files, scanned files, cards found, last path,
completion, and normal errors. Add `--progress` to print a small stderr
heartbeat from the same status stream. If a normal write or rename error occurs
after rendering, the partial report is kept at `<out>.partial`; if the process
is interrupted before rendering, the latest status sidecar is the durable
artifact.

For large or mixed repositories, bound the scan with repo-only file selection
controls:

```bash
unsafe-review repo \
  --root . \
  --include 'src/**/*.rs' \
  --include 'packages/**/*.rs' \
  --exclude 'vendor/**' \
  --exclude 'build/**' \
  --exclude '**/generated/**' \
  --format markdown \
  --out target/unsafe-review/repo-posture.md
```

`--include` and `--exclude` are repeatable glob filters over root-relative Rust
paths. Repo discovery respects gitignore files by default; use
`--no-respect-gitignore` only when the review intentionally includes ignored
Rust files. Repo discovery also skips common large or generated directories by
default: `.git`, `.github`, `.unsafe-review*`, `target`, `node_modules`,
`vendor`, `build`, `dist`, and `generated`.

Use `--list-files` as a dry run before scanning a large repo:

```bash
unsafe-review repo \
  --root . \
  --include 'src/**/*.rs' \
  --exclude '**/generated/**' \
  --list-files
```

`--list-files` prints the selected root-relative Rust files and exits without
running analysis. `--max-files <n>` truncates the selected file list after
sorting, so it bounds both `--list-files` output and repo analysis input.

Badge JSON reports open review gaps, not raw unsafe usage and not safety status:

```bash
unsafe-review badges --out badges/
```

The badge output is repo posture evidence only. It is not a safety badge.

Compare two saved `unsafe-review --format json` snapshots:

```bash
unsafe-review outcome \
  --before target/unsafe-review/before.json \
  --after target/unsafe-review/after.json \
  --format markdown \
  --out target/unsafe-review/outcome.md
```

Outcome comparison is read-only. It compares existing `ReviewCard` identities,
classes, operation expressions and families, missing-evidence counts, next
actions, and saved witness receipt strength from the supplied snapshots. The
report includes a compact reviewer delta with new, resolved, improved,
regressed, receipt-movement, and top-remaining-gap context. It does not rerun
analysis, run witnesses, post policy decisions, or claim repository safety.

## Witness Receipts

Imported receipts attach external witness evidence to exact `ReviewCard`
identities:

```text
.unsafe-review/receipts/*.json
```

A receipt must include exact counted `card_id`, `tool`, `strength`, `author`,
`recorded_at`, `expires_at`, and optional command/limitations details. Matching
receipts whose `tool` matches the card's routed witness tools and whose
`strength` is `ran`, `test_targeted`, or `site_reached` mark witness evidence
present, but they do not discharge missing contracts, guards, or reach evidence.
A `configured` receipt or a receipt whose tool is not routed for the current
card remains valid audit metadata and does not remove the missing witness gap.

The receipt JSON shape is backed by `unsafe_review_core::WitnessReceipt`, so SDK
consumers and future native adapters should produce that same schema rather than
a parallel receipt format.

Generate a receipt JSON template after a witness has been run outside
`unsafe-review`:

```bash
unsafe-review receipt template <card-id> \
  --tool miri \
  --strength ran \
  --author core/fixtures \
  --recorded-at 2026-05-18T00:00:00Z \
  --expires-at 2026-08-18 \
  --summary "focused witness passed" \
  --command "cargo +nightly miri test read_header" \
  --limitation "fixture only" \
  --out .unsafe-review/receipts/miri.json
```

The template command validates the receipt shape and writes JSON. It still does
not run the witness command. When `--command` is present, the generated JSON also
includes a stable `command_hash` for drift checks; the hash is not proof that the
command ran.

Import a receipt from saved Miri output after Miri has been run outside
`unsafe-review`:

```bash
unsafe-review receipt import-miri <card-id> \
  --log target/miri-read-header.log \
  --author core/fixtures \
  --recorded-at 2026-05-18T00:00:00Z \
  --expires-at 2026-08-18 \
  --command "cargo +nightly miri test read_header" \
  --limitation "fixture only" \
  --out .unsafe-review/receipts/miri.json
```

The Miri adapter reads saved output and writes a `tool = "miri"` receipt with
`strength = "ran"` only when the output contains `test result: ok` and no
failure marker. It does not run Miri, parse native UB diagnostics into cards, or
claim site reach.

Import a receipt from saved `cargo-careful` output after `cargo-careful` has
been run outside `unsafe-review`:

```bash
unsafe-review receipt import-careful <card-id> \
  --log target/careful-read-header.log \
  --author core/fixtures \
  --recorded-at 2026-05-18T00:00:00Z \
  --expires-at 2026-08-18 \
  --command "cargo +nightly careful test read_header" \
  --limitation "fixture only" \
  --out .unsafe-review/receipts/careful.json
```

The `cargo-careful` adapter reads saved output and writes a
`tool = "cargo-careful"` receipt with `strength = "ran"` only when the output
contains `test result: ok` and no failure marker. It does not run
`cargo-careful`, parse diagnostics into cards, or claim site reach.

Import a receipt from saved sanitizer output after the sanitizer run happened
outside `unsafe-review`:

```bash
unsafe-review receipt import-sanitizer <card-id> \
  --tool asan \
  --log target/asan-read-header.log \
  --author core/fixtures \
  --recorded-at 2026-05-18T00:00:00Z \
  --expires-at 2026-08-18 \
  --command "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header" \
  --limitation "fixture only" \
  --out .unsafe-review/receipts/asan.json
```

The sanitizer adapter accepts `asan`, `msan`, `tsan`, or `lsan` as the explicit
receipt tool. It reads saved output and writes a receipt with `strength = "ran"`
only when the output contains `test result: ok` and no failure marker. It does
not run a sanitizer, parse sanitizer diagnostics into cards, or claim site
reach.

Import a receipt from saved Loom or Shuttle output after the concurrency witness
run happened outside `unsafe-review`:

```bash
unsafe-review receipt import-concurrency <card-id> \
  --tool loom \
  --log target/loom-shared-cell.log \
  --author core/fixtures \
  --recorded-at 2026-05-18T00:00:00Z \
  --expires-at 2026-08-18 \
  --command "cargo test shared_cell_loom -- --nocapture" \
  --limitation "fixture only" \
  --out .unsafe-review/receipts/loom.json
```

The concurrency adapter accepts `loom` or `shuttle` as the explicit receipt
tool. It reads saved output and writes a receipt with `strength = "ran"` only
when the output contains `test result: ok` and no failure marker. It does not
run Loom or Shuttle, infer site reach, or claim that all scheduler interleavings
or callers are covered.

Import a receipt from saved Kani or Crux proof output after the proof run
happened outside `unsafe-review`:

```bash
unsafe-review receipt import-proof <card-id> \
  --tool kani \
  --log target/kani-byte-to-bool.log \
  --author core/fixtures \
  --recorded-at 2026-05-18T00:00:00Z \
  --expires-at 2026-08-18 \
  --command "cargo kani --harness byte_to_bool_harness" \
  --limitation "fixture only" \
  --out .unsafe-review/receipts/kani.json
```

The proof adapter accepts `kani` or `crux` as the explicit receipt tool. It reads
saved output and writes a receipt with `strength = "ran"` only when the output
contains a conservative verification-success marker and no failure marker. It
does not run Kani or Crux, infer site reach, or claim proof beyond the recorded
harness/output scope.

Validate imported receipt files without running analysis:

```bash
unsafe-review receipt validate --root .
```

This checks `.unsafe-review/receipts/*.json` with the same validation path used
by `check` and reports how many receipts are importable.

Audit imported receipts against the current ReviewCard set without running
witnesses:

```bash
unsafe-review receipt audit \
  --root . \
  --base origin/main \
  --format markdown \
  --out target/unsafe-review/receipt-audit.md
```

The audit reports matched, unmatched, stale, expired, wrong-identity,
wrong-tool, weaker-than-required, command-hash-mismatch, duplicate, and invalid
receipt metadata. Matched receipts include current ReviewCard operation
expression, operation family, missing-count, next-action context, routed witness
tools, saved `summary`, saved `author`, saved `recorded_at` timestamp, and the
saved `command_hash` and per-receipt limitations when present so receipt
evidence does not hide remaining gaps or saved scope limits. The summary,
author, command hash, and limitations are saved metadata only, not proof that
the command ran or covered the unsafe site. It is advisory only: it does not
execute witness commands, infer site reach, make policy decisions, or claim
safety. A receipt entry gets `imports_witness_evidence` only when it is a
current-card match with a routed tool, saved-run strength, no expiry, no
validation error, and no duplicate for that card. JSON and Markdown output
include report-level limitations that keep the saved-metadata boundary explicit.
When a receipt matches a card, the ReviewCard witness evidence summary also
keeps the saved command hash visible when present.

`unsafe-review` imports receipts. It does not run Miri, `cargo-careful`,
sanitizers, Loom, Shuttle, Kani, or Crux by default.

## Doctor

Run a lightweight environment check:

```bash
unsafe-review doctor
```

`doctor` reports first-install signals: workspace root, Git availability,
whether `origin/main` is visible, Cargo metadata readiness, artifact directory
writability, witness tool availability or configuration hints, advisory policy,
and the trust boundary. Missing witness tools are reported, not treated as a
default failure. The command does not run Miri, `cargo-careful`, sanitizers,
Loom, Shuttle, Kani, Crux, or any witness test.

## Flag Forms

Flags may use either form:

```bash
unsafe-review check --root fixtures/raw_pointer_alignment --format json
unsafe-review check --root=fixtures/raw_pointer_alignment --format=json
```

Use `--out` to write artifacts without printing them:

```bash
unsafe-review check --diff change.diff --format sarif --out target/unsafe-review/cards.sarif
```
