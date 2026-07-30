# First-hour guide

This guide is for a Rust maintainer who has heard `unsafe-review` exists and
wants to spend an hour deciding whether to adopt it. It assumes nothing about
source-of-truth specs, support tiers, dogfood receipts, or the swarm/source
operating model. It walks one CLI user path from install to a credible review
action.

If you only want install and one card, [docs/FIRST_USE.md](FIRST_USE.md) is
the shorter walkthrough. This guide goes one step further: it shows what to do
after the first card lands.

For the repeatable loop from a changed unsafe seam to fix, external witness
receipt, receipt audit, and before/after comparison, see
[Find and fix UB-risk review seams](FIND_AND_FIX_UB.md).

## Trust boundary first

`unsafe-review` is static unsafe-contract review. It finds unsafe Rust changes
missing a safety contract, guard, test, or witness. It does not prove memory safety, does not claim UB-free status, does not run Miri by default, does not post comments, does not edit source, and does not enable blocking policy. A finding means:

```text
This changed unsafe-adjacent seam is missing review evidence.
```

It does not mean:

```text
This code is UB.
This repository is unsafe.
Miri failed.
Miri passed.
```

A no-card result means no changed unsafe-review gap was found in this diff. It
is not proof the repository is safe, UB-free, Miri-clean, or that any unsafe
site executed.

## Step 1 — Install

```bash
cargo install unsafe-review --locked
unsafe-review --version
```

The `unsafe-review` crate is the maintainer install handle. Programmatic users
should depend on `unsafe-review-core` instead.

## Step 2 — Check your environment

```bash
unsafe-review doctor
```

`doctor` reports Git/base-ref visibility, Cargo metadata readiness, artifact
directory writability, and witness-tool hints. Missing witness tools (Miri,
`cargo-careful`, sanitizers, Loom, Shuttle, Kani, Crux) are informational.
`doctor` does not run witnesses and does not make policy decisions.

## Step 3 — Run on your PR

From the branch you want to review, against your main branch:

```bash
unsafe-review pr
```

This auto-detects the repository root and default base ref, then writes the
advisory PR bundle. If your base ref cannot be detected, pass it explicitly
with `unsafe-review pr --base origin/main`.

```text
target/unsafe-review/review-kit.json
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/github-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/receipt-audit.md
target/unsafe-review/receipt-audit.json
target/unsafe-review/policy-report.json
target/unsafe-review/policy-report.md
target/unsafe-review/manual-candidates.json
target/unsafe-review/lsp.json
target/unsafe-review/manual-repair-queue.json
target/unsafe-review/tokmd-packets.json
target/unsafe-review/usefulness-telemetry.json
target/unsafe-review/repair-queue.json
target/unsafe-review/unsafe-review-gate.json
```

`pr` is artifact-only: it does not run witness tools, post comments, edit
source, or enforce a blocking policy. It is safe to run on any branch.

For brownfield adoption, from the intended clean base/default branch before
feature changes, you can record the current open actionable gaps as a debt floor
before opting into a no-new-debt policy. Do not run this from the PR branch
being reviewed; otherwise new branch gaps can become baseline debt:

```bash
unsafe-review baseline init --root . --dry-run --format json
```

Review the proposal first. This preview writes nothing. To author the
baseline after review, run `unsafe-review baseline init --root .`; that writes
`policy/unsafe-review-baseline.toml` and
`policy/unsafe-review-baseline-snapshot.toml`. Review those files before
committing them. Baseline entries are debt records, not safety records, witness
results, or UB-free status.

## Step 4 — Open the PR summary

```bash
open target/unsafe-review/pr-summary.md   # or your editor of choice
```

The summary is the reviewer front panel. It names the highest-priority changed
unsafe-review gaps, their missing evidence, and a recommended next action per
card. It also prints `Explain top card` and `Agent packet` commands for the most
actionable card, plus a `receipt-audit.md` cue for saved witness receipt
metadata. The receipt cue is metadata-only; it does not mean a witness was run.

If the summary says no changed unsafe-review gaps were found, you can stop
here. That is the normal result for safe-only PRs and is not a safety claim.

## Step 5 — Explain one card

Run the top-card command printed in the summary, or copy any card id from
`pr-summary.md` or `cards.json`:

```bash
unsafe-review explain <card-id>
```

`explain` answers the reviewer-first questions: what unsafe operation changed,
what obligation matters, what evidence was found, what evidence is missing,
what would resolve the card, what would not resolve it, which witness route
fits, and what `unsafe-review` is not claiming. For fixture-backed examples,
see [Explain examples](explanation/explain-examples.md).

## Step 6 — Check support posture before you ask for changes

```bash
unsafe-review support
```

`support` prints the experimental, advisory, and deferred boundaries for each
analyzer family. It is the place to confirm the finding is from a supported
analyzer surface before you ask an author to add a guard or contract.

## Step 7 — Optional: read the witness plan

```bash
open target/unsafe-review/witness-plan.md
```

The witness plan suggests credible next tools (Miri, `cargo-careful`,
sanitizers, Loom, Shuttle, Kani, Crux) per card. Routes describe what each
tool can and cannot show. `unsafe-review` does not claim a witness ran unless
a matching receipt is attached.

## Step 8 — Optional: hand a card to an agent

```bash
unsafe-review context <card-id> --json
```

`context` emits a bounded repair packet for an LLM or agent: missing evidence,
allowed repairs, do-not-do rules, verify commands, stop conditions, and the
trust boundary. The packet is copy-only; `unsafe-review` does not edit source.

## Step 9 — Optional: preview editor data

The PR bundle includes a saved editor projection:

```text
target/unsafe-review/lsp.json
```

It is read-only data derived from the same `ReviewCard`s as the PR summary. It
shows the diagnostics, hovers, and command payloads a future editor adapter
will consume. See [Saved LSP JSON workflow](editor/saved-lsp-json.md) for the
current editor-adjacent path. There is no live LSP server requirement and no
editor extension is required to use `unsafe-review`.

## Step 10 — Optional: try the deterministic fixture

From a local checkout of `EffortlessMetrics/unsafe-review`, you can run the
bundled smoke fixture to see one `guard_missing` raw pointer alignment card:

```bash
unsafe-review pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --out-dir target/unsafe-review-fixture
```

## Read-only external repo scan and the witness boundary

You can run `unsafe-review` against any Rust repository you have checked out
locally. The tool is read-only: it never edits source, never posts comments, and
never runs witness tools.

**Step 1 — scan the external repo:**

```bash
unsafe-review pr --root /path/to/external/repo
```

Or, to diff against a specific base commit:

```bash
unsafe-review pr \
  --root /path/to/external/repo \
  --base HEAD~20
```

This writes the advisory bundle under
`/path/to/external/repo/target/unsafe-review/` (or use `--out-dir` to redirect
it). The output is advisory findings — not proof of unsafety.

For a public GitHub PR that is not checked out as the current branch, prefer an
exact local checkout over a display diff. First capture the base branch name and
immutable base/head SHAs:

```bash
gh pr view 827 --repo tokio-rs/bytes --json baseRefName,baseRefOid,headRefOid
unsafe-review pr-setup \
  --repo tokio-rs/bytes \
  --number 827 \
  --base-ref <base-ref-name> \
  --base-sha <base-sha> \
  --head-sha <head-sha> \
  --root /path/to/external/repo \
  --out-dir /absolute/path/to/review-kit \
  --diff-out /absolute/path/to/change.diff
```

`pr-setup` prints the copyable checkout, `unsafe-review pr`, and raw diff
commands without running `gh`, `git`, witnesses, comments, or source edits. The
direct commands it prints are:

```bash
git -C /path/to/external/repo fetch origin <base-ref-name> pull/<number>/head
git -C /path/to/external/repo checkout --detach <head-sha>
unsafe-review pr \
  --root /path/to/external/repo \
  --base-sha <base-sha> \
  --head-sha <head-sha> \
  --out-dir /absolute/path/to/review-kit
mkdir -p /absolute/path/to
git -C /path/to/external/repo diff --binary --full-index --output=/absolute/path/to/change.diff <base-sha>...<head-sha>
unsafe-review pr \
  --root /path/to/external/repo \
  --diff /absolute/path/to/change.diff \
  --out-dir /absolute/path/to/review-kit
```

Use the first `unsafe-review pr` command for checkout-based analysis. Use the
final `--diff` form when you need the saved raw patch route for receipts or
replay.

Use the `baseRefName` value for `<base-ref-name>`, `baseRefOid` for
`<base-sha>`, and `headRefOid` for `<head-sha>`. The `pull/<number>/head` fetch
gets the PR head through GitHub's PR ref, which works for fork PRs without
relying on raw-SHA fetch support from `origin`. `--head-sha` still keeps the
exact checked-out head visible and validated.
Avoid copying from rendered GitHub diff views, and avoid shell redirection for
saved patches on older Windows PowerShell: it can re-encode the file while
leaving it readable to humans. The printed raw-diff path lets Git write the file
with `git diff --output=<path>` and uses an absolute output path because
`git -C` changes directories. The printed `--out-dir` keeps the review bundle
anchored with the pilot artifacts. A valid saved patch still contains
`diff --git`, `---`, `+++`, and `@@` lines.

**Step 2 — understand the advisory boundary:**

A card means:

```text
This unsafe-adjacent change is missing a safety contract, guard, test, or witness.
```

It does not mean:

```text
This code is UB.
Miri found a bug.
The code is memory-unsafe.
```

A no-card result does not mean the repo is safe, UB-free, or Miri-clean.

**Step 3 — attach a witness receipt only if you have external evidence:**

A witness receipt is a SEPARATE, explicit attachment. It records metadata from
an external tool run (Miri, `cargo-careful`, a sanitizer, Loom, Shuttle, Kani,
or Crux) that you ran yourself. `unsafe-review` does not run these tools by
default and does not claim they ran unless a receipt is attached.

To attach a receipt after running a witness tool externally, use
`unsafe-review receipt import` (see `CLI.md` for the receipt format). Without
an attached receipt, every card and every no-card result is advisory only — no
Miri-clean or UB-free claim is warranted.

## After the first hour

The CLI walkthrough is the maintainer surface. After the first hour, common
next steps are:

- Wire `unsafe-review` into CI as an advisory PR job: see
  [docs/ci/UB_RISK_REVIEW_CI.md](ci/UB_RISK_REVIEW_CI.md) for the cookbook,
  [docs/ci/PR_CI.md](ci/PR_CI.md) for the lane model and
  `.github/examples/unsafe-review-first-pr.yml` for a copy-paste workflow.
- Read [CLI reference](CLI.md) for receipt import, policy report, and outcome
  comparison commands.
- Use [Find and fix UB-risk review seams](FIND_AND_FIX_UB.md) when a card needs
  a bounded repair, external witness receipt, and outcome comparison.
- Read [ReviewCard explanation](explanation/review-cards-and-trust-boundary.md)
  for the canonical analyzer unit.
- Check [Support summary](status/SUPPORT_SUMMARY.md) for which surfaces are
  current product promises and which are still experimental.

## Non-goals of the first hour

The first hour does not include:

- enabling automatic PR comment posting,
- enabling a default blocking policy,
- editing source automatically,
- running Miri, `cargo-careful`, Loom, Shuttle, Kani, Crux, or sanitizers by
  default,
- installing the (still-planned) VS Code or Open VSX editor extension,
- making any safety, UB-free, Miri-clean, site-execution, or calibrated
  precision/recall claim.

A finished first hour ends with one credible review action: ask the author to
add a guard, add a contract, run a targeted witness, or accept the change with
recorded evidence — not with `unsafe-review` having made the decision for you.
