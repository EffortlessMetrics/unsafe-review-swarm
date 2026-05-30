# First-hour guide

This guide is for a Rust maintainer who has heard `unsafe-review` exists and
wants to spend an hour deciding whether to adopt it. It assumes nothing about
source-of-truth specs, support tiers, dogfood receipts, or the swarm/source
operating model. It walks one CLI user path from install to a credible review
action.

If you only want install and one card, [docs/FIRST_USE.md](FIRST_USE.md) is
the shorter walkthrough. This guide goes one step further: it shows what to do
after the first card lands.

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
unsafe-review first-pr --base origin/main
```

This writes the advisory PR bundle:

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

`first-pr` is artifact-only: it does not run witness tools, post comments, edit
source, or enforce a blocking policy. It is safe to run on any branch.

## Step 4 — Open the PR summary

```bash
open target/unsafe-review/pr-summary.md   # or your editor of choice
```

The summary is the reviewer front panel. It names the highest-priority changed
unsafe-review gaps, their missing evidence, and a recommended next action per
card. It also prints `Explain top card` and `Agent packet` commands for the most
actionable card.

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

The first-pr bundle includes a saved editor projection:

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
unsafe-review first-pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --out-dir target/unsafe-review-fixture
```

## After the first hour

The CLI walkthrough is the maintainer surface. After the first hour, common
next steps are:

- Wire `unsafe-review` into CI as an advisory PR job: see
  [docs/ci/PR_CI.md](ci/PR_CI.md) for the lane model and
  `.github/examples/unsafe-review-first-pr.yml` for a copy-paste workflow.
- Read [CLI reference](CLI.md) for receipt import, policy report, and outcome
  comparison commands.
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
