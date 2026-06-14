# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`unsafe-review` is an advisory static-review tool for Rust PRs. The product sentence: *unsafe-review finds unsafe Rust changes missing a safety contract, guard, test, or witness.* It does not prove unsafe code sound, and every output surface must preserve that trust boundary: no memory-safety proof, no UB-free claim, no Miri-clean claim unless a matching witness receipt is attached, advisory by default (no witness execution, no posted comments, no source edits, no blocking policy). xtask gates and e2e tests enforce this wording — do not add output text that overclaims.

## Read first

- `AGENTS.md` — the agent operating contract. It governs command style (prefix local commands with `rtk`), repository roles, worktree/branch hygiene, model routing (cheap discovery/verification, mid-tier implementation, top-tier arbitration — project subagent roles in `.claude/agents/`), PR queue discipline, and product boundaries. This file summarizes; AGENTS.md wins on conflict.
- Source-of-truth stack for choosing and scoping work: `.rails/goals/active.toml` → linked plan item → linked spec in `docs/specs/`. Make one PR-sized change and run the proof commands the plan item lists.

## Repository roles

`unsafe-review-swarm` develops, `unsafe-review` publishes. Remote `origin` is the swarm workbench (routine PRs land here, targeting `main`); remote `public` is `EffortlessMetrics/unsafe-review`, the source-of-record/release repo — only curated promotions, release prep, and hotfixes go there. Before routine work, check sync posture with `cargo run --locked -p xtask -- source-divergence`. If the primary checkout is dirty, work from a fresh worktree off `origin/main` instead of editing through it.

## Commands

```bash
cargo fmt --all --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- check-pr        # docs/policy/fixtures/calibration/dogfood/spec gates
```

Run a single test (most coverage lives in per-crate `tests/e2e.rs` files that drive the built binary against `fixtures/`):

```bash
cargo test -p unsafe-review --test e2e <test_name> --locked
cargo test -p unsafe-review-cli --test e2e <test_name> --locked
```

Run the tool itself against the bundled smoke fixture:

```bash
cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff
```

`cargo run -p xtask -- help` lists all gates. Beyond the `check-pr` bundle, notable ones: `source-divergence` / `check-source-sync` (swarm↔source drift), `check-advisory-artifacts <dir>` / `check-first-pr-artifacts <dir>` (validate generated output bundles), `check-spec-status`, `check-goals`, `dogfood-usefulness`.

Fuzzing is manual, not part of the PR gate: `cargo fuzz run analyze` (see `docs/FUZZING.md`). Toolchain is pinned (`rust-toolchain.toml`): Rust 1.95.0, edition 2024.

CI has exactly one required check: the deterministic core gate (the command sequence above). An advisory ub-review LLM lane rides along in the same job and never blocks the merge. Do not turn advisory unsafe-review findings into default CI failures; read `docs/specs/UNSAFE-REVIEW-SPEC-0024-ci-design.md` before editing CI, workflows, or PR-artifact surfaces.

Commit subjects follow `area: summary` (e.g. `cli:`, `analysis:`, `docs(specs):`, `sync:`, `ci:`), lowercase, imperative.

## Lint posture (matters when writing code)

Workspace lints deny, among others: `unsafe_code` (forbidden), `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`, `clippy::todo`, `clippy::unimplemented`, `clippy::unreachable`, `clippy::allow_attributes_without_reason`. Return `Result` and propagate errors; never unwrap. Any `#[allow]` needs a `reason`.

## Architecture

Three published crates plus an automation crate, strictly layered:

```text
unsafe-review          # product facade / install handle (thin)
  -> unsafe-review-cli # command parsing, execution, UX rendering
      -> unsafe-review-core # SDK: domain types, analyzer, policy, output schemas
xtask                  # repo automation gates, not product surface
```

Boundary doctrine: design seams like microcrates, implement most as module families inside `unsafe-review-core`, publish only seams that deserve a support promise.

### Core pipeline (`crates/unsafe-review-core/src/`)

```text
input scope (input/: diff parsing, workspace discovery)
-> unsafe seam extraction (analysis/scanner, syntax.rs)
-> hazard classification (analysis/classify.rs, domain/hazard.rs)
-> safety obligation mapping (analysis/obligations)
-> contract + discharge evidence mining (analysis/evidence)
-> test reach estimation, witness routing, receipts (analysis/witness.rs, receipts.rs)
-> review-card classification (domain/review_card.rs, classification.rs)
-> output projection (output/: json, markdown, sarif, human, lsp, agent,
   comment_plan, witness_plan, outcome, badges, confirmation, gate_manifest,
   policy_report, receipt_audit, repair_queue)
```

Per-operation-family detection lives in dedicated `analysis/` modules (`transmute_operation.rs`, `vec_operation.rs`, `ffi_boundary.rs`, ...), registered through `domain/operation.rs`.

**The ReviewCard is the single truth object.** Every surface — CLI output, JSON, PR summary, SARIF, LSP diagnostics, agent packets, badges, baselines, suppressions, witness receipts — must project from the same card. No second truth surface is allowed.

Evidence must be obligation-level: a length guard does not discharge alignment, a `SAFETY` comment is not a guard, and a targeted test is not site-execution proof unless a receipt proves it. Optimize card correctness before analyzer breadth.

Stability posture: stable-only Rust, no `rustc_private`/MIR. The analyzer is source-text heuristic by design.

### Fixtures, calibration, and dogfood (the evidence system)

- `fixtures/<name>/` — each is a tiny crate (`Cargo.toml`, `src/lib.rs`, `change.diff`) proving one detection or false-positive control. Naming convention: `<operation>_<scenario>` with `_not_guard` / `_no_cards` suffixes for negative controls.
- `policy/calibration.toml` — manifest mapping every fixture to expected cards, class, operation family, hazard, and support tier. New fixtures require a calibration entry (`check-fixtures` + `check-calibration` enforce this), and new operation families must be registered in the registry appendix under `docs/specs/appendices/` — `check-calibration` cross-checks the registry table against `domain/operation.rs`, `analysis/obligations.rs`, `domain/hazard.rs`, and `domain/witness.rs`.
- `docs/dogfood/corpus.toml` + `index.json`/`index.md` — evidence from running against real crates and PR diffs; validated by `check-dogfood`.
- `policy/*.toml` — allowlist ledgers (no-panic, non-Rust files, executables, workflows, network, etc.) validated by `check-policy`. Adding e.g. a new workflow file requires a ledger entry.

**Fixture-suite blindness.** A fixture suite encodes the author's assumptions, so it is blind to assumptions the author did not know they were making. The wave-1 fixtures for every new detector historically placed operations inside `unsafe { }` blocks — correct by the spec, but masking the possibility that a detector would fire on safe-context code entirely. Real-crate dogfood on fresh, unseen code is the check that fixture suites cannot supply: it exercises paths the author never thought to encode as a test case. New detectors need real-context adversarial negative controls (code that resembles the target pattern but is in safe context, inside a comment, or is a function definition), and a fresh-crate dogfood run is the required pre-release validation step before promotion.

### Documentation system (gated, not optional)

Specs in `docs/specs/` define behavior; ADRs in `docs/adr/` record decisions; `docs/status/SUPPORT_TIERS.md` is the claim-to-proof ledger and `SUPPORT_SUMMARY.md` the posture summary. `check-docs` and `check-support-tiers` enforce required docs, front-door wording, and that every claimed tier names its proof. Behavior changes typically need spec/status updates to pass `check-pr`. Do not invent missing claims — if proof is missing, the claim stays advisory/experimental.
