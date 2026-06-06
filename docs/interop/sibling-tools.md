# Sibling tools and bidirectional learning

`unsafe-review` is one of a series of **deterministic, fast, useful static PR
tools** that share interfaces, are composed by the same CI gate, and
deliberately learn from each other. Each is cheap, runs on a diff, and emits
trusted coverage artifacts without executing repo code or issuing a verdict.
This document makes that relationship explicit so a contributor in any of the
repos can see what is shared, what is converging, and how to keep the flow
going. Each sibling repo carries a mirror of this doc pointing back at the same
convergence ledger.

## The family

| Tool | Repo | Role (coverage instrument) |
|---|---|---|
| `unsafe-review` | `EffortlessMetrics/unsafe-review-swarm` | unsafe-contract coverage: which unsafe seams are reviewable, what evidence exists/is missing |
| `ripr` | `EffortlessMetrics/ripr-swarm` | mutation / weak-oracle exposure coverage |
| `cargo-allow` | `EffortlessMetrics/cargo-allow` | owned exception ledger (unsafe/panic/lint/etc. allow entries) |
| `tokmd` | `EffortlessMetrics/tokmd-swarm` | token-aware repo receipts and PR context packets |
| `ub-review` | `EffortlessMetrics/ub-review` | **CI gate** — composes the family (configurable) + LLM lanes; owns PR analysis, review, posting, and the blocking decision |

Each family member is a deterministic, fast, static PR tool — an instrument.
`ub-review` is the CI gate built on top of them: it keeps a repo's mandatory CI
surface clean and simple, then dynamically adds the PR-relevant gate items by
composing the family (the whole set, a user-configured subset, or others) and
running LLM lanes over their coverage artifacts for analysis, review, and
gating. None of the family members is itself the gate or the LLM reviewer (see
UNSAFE-REVIEW-SPEC-0028). They are complementary, not competing, and a
capability proven in one is expected to flow to the others.

## Shared contracts (converging)

These are the interfaces the family is aligning on so `ub-review` can route all
sensors uniformly instead of special-casing each:

- **Sensor CLI shape**: `--root`, `--base`, `--diff`, `--head`, `--format`,
  `--out`. Diff/PR context flows in the same way to every sensor.
- **`<tool>-gate.json` manifest envelope**: a versioned (`schema_version`)
  manifest with `status`, a summary count block, artifact pointers, and a
  `trust_boundary`, so the orchestrator routes by schema, not by scraping
  stdout/markdown.
- **Ledger evidence taxonomy and dialect marker**: typed evidence prefixes
  (`test:`, `doc:`, `spec:`, `adr:`, `ripr:`, `unsafe-review:`, `coverage:`,
  `issue:`, `pr:`), a `policy = "<tool>"` dialect marker, owner/classification/
  lifecycle discipline, and one settled `schema_version` form (integer vs
  string).
- **Coverage-movement vocabulary**: `new` / `worsened` / `resolved` /
  `inherited` reported as posture against a baseline, with diff-scoped
  attribution; the orchestrator decides blocking.
- **Trust-boundary and advisory discipline**: advisory by default; conservative
  vocabulary; explicit "this does not prove ..." no-finding wording; no
  overclaim (no proof / UB-free / Miri-clean / site-execution / calibrated
  precision-recall / policy-readiness).
- **Spec lifecycle dashboard + wording-contract verifier**: machine-checked
  spec status with proof commands, and a verifier that rejects overclaim drift.

## Bidirectional learning ledger

Live convergence items. Direction = which tool is ahead; the receiving repo
owns the tracking issue. Keep this table current as items land.

| Capability | Ahead | Receiving | Tracking | Status |
|---|---|---|---|---|
| Versioned gate manifest + baseline-debt-delta schema | ripr | unsafe-review | unsafe-review-swarm #1522 | open |
| Multi-mode gate (visible-only → calibrated) | ripr | unsafe-review | unsafe-review-swarm #1522 | open |
| Canonical `new_unsuppressed` counter for threshold consumers | ripr | (consumer contract) | ripr-swarm #1038 | open |
| Exception-ledger rigor: typed evidence, classification, structural selectors, ownership, dialect marker | cargo-allow | unsafe-review | unsafe-review-swarm #1523 | open |
| `schema_version` integer-vs-string convergence | — | family-wide | cargo-allow #1465, tokmd-swarm #224 | open |
| Machine-checked spec-status dashboard | unsafe-review | ripr | ripr-swarm #1040 | open |
| No-finding wording-contract verifier | unsafe-review | ripr | ripr-swarm #1040 | open |
| Diff-first consumer contract alignment | unsafe-review | ripr | ripr-swarm #1041 | open |
| Coverage-movement vocabulary (new/worsened/resolved/inherited) | unsafe-review | cargo-allow | cargo-allow #1471 | open |
| tokmd-packets input-schema ownership + `--from-packets` consumer | unsafe-review (producer) ↔ tokmd (consumer) | tokmd | tokmd-swarm #222 | open |
| `check-local-context` / limited-runtime vocabulary | ripr | unsafe-review | unsafe-review-swarm #1520 | open |
| Pre-guard scratch GC for shared CI runners | ripr | unsafe-review | unsafe-review-swarm #1519 | open |

## Standing process

When work in one tool surfaces something a sibling should learn (interface
alignment, manifest envelope, ledger/evidence schema, movement vocabulary,
trust-boundary discipline, receipts/confirmation, spec rails):

1. File the issue in the **receiving** tool's `-swarm` repo (the one that should
   adopt), tagged with **direction**, **evidence** (file:line / artifact), and a
   one-line **proposal**.
2. If a shared contract is involved, co-design it across both repos rather than
   asserting one side's schema — cross-link the sibling issue.
3. Add a row to this ledger (in both repos' mirror of this doc).
4. Do not duplicate an existing issue; comment with the concrete contract
   instead.

## Trust boundary

Cross-pollination changes interfaces and rigor, not claims. Every sibling tool
stays advisory by default and within its own trust boundary; sharing a manifest
envelope or a ledger schema never lets one tool assert another's proof. The
family converges on honesty discipline as much as on interfaces.
