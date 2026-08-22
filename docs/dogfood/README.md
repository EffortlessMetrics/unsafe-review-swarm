# Dogfood Corpus

This directory records the selected real-crate dogfood corpus for
`unsafe-review`.

The corpus is advisory evidence. It records repeatable targets, commands, and
expected artifact paths for real Rust crates, PR diffs, and separately indexed
fixture controls. It is not a release claim, not calibrated precision/recall
measurement, and not memory-safety proof.

The manifest is [`corpus.toml`](corpus.toml). The human-facing index is
[`index.md`](index.md), with a machine-readable companion at
[`index.json`](index.json). Reviewer usefulness notes live in
[`usefulness-notes.md`](usefulness-notes.md). The external pilot usefulness
rollup is [external pilot usefulness](reports/external-pilot-usefulness-rollup.md).
Dogfood report labels are defined
in the [`triage taxonomy`](triage-taxonomy.md). Follow-up work seeds are tracked
in [`follow-up-seeds.md`](follow-up-seeds.md). Bun stable-byte follow-up seeds
are tracked in [`stable-byte-follow-up-seeds.md`](stable-byte-follow-up-seeds.md)
with labels from the
[`stable-byte triage taxonomy`](stable-byte-triage-taxonomy.md). Bun diff-first
inventory requirements for `ripr` are tracked in
[`ripr-bun-diff-first-requirements.md`](ripr-bun-diff-first-requirements.md).
Bun packet preset requirements for `tokmd` are tracked in
[`tokmd-bun-packet-presets.md`](tokmd-bun-packet-presets.md).
Presentation-density observations follow the
[`dogfood density receipt schema`](density-receipts/README.md).
Reviewer judgment files follow the [`dogfood judgment schema`](judgments/README.md).
External read-only pilot receipts follow the
[`external pilot receipt schema`](pilots/README.md).
The generated per-label usefulness count rollup lives in
[`USEFULNESS.md`](USEFULNESS.md); regenerate it with
`cargo run --locked -p xtask -- dogfood-usefulness`.
Card-scoped agent repair dry runs follow the
[`agent repair experiment protocol`](agent-repair-experiments.md).
A narrative summary of real-world findings across the seven dogfood crates
lives in [`REAL_WORLD_FINDINGS.md`](REAL_WORLD_FINDINGS.md).

Snapshot reports:

- [2026-08-20 ub-review exact producer/consumer smoke (#2116)](reports/2026-08-20-ub-review-consumer-smoke.md)
- [2026-08-14 holdout rotation closeout](reports/2026-08-14-holdout-rotation-closeout.md)
- [2026-08-10 tokmd consumer five-preset receipt](reports/2026-08-10-tokmd-consumer-five-preset-receipt.md)
- [2026-08-10 typed repair candidate agent evaluation](reports/2026-08-10-repair-candidate-agent-evaluation.md)
- [2026-08-10 cargo-allow #541 reproduction (#1890)](reports/2026-08-10-cargo-allow-541-reproduction.md)
- [2026-08-08 cargo-allow current-main reproduction](reports/2026-08-08-cargo-allow-current-main.md)
- [2026-08-10 serde holdout](reports/2026-08-10-serde-holdout.md)
- [2026-07-29 simdutf8 holdout](reports/2026-07-29-simdutf8-holdout.md)
- [2026-07-30 crossbeam holdout](reports/2026-07-30-crossbeam-holdout.md)
- [2026-07-30 rkyv holdout](reports/2026-07-30-rkyv-holdout.md)
- [2026-07-30 slab holdout](reports/2026-07-30-slab-holdout.md)
- [2026-07-29 portable-atomic holdout](reports/2026-07-29-portable-atomic-holdout.md)
- [2026-07-19 holdout contract audit](reports/2026-07-19-holdout-contract-audit.md)
- [2026-06-19 generalization validation closeout](reports/2026-06-19-generalization-validation-closeout.md)
- [2026-06-19 initial holdout report](reports/2026-06-19-initial-holdout-report.md)
- [2026-06-18 residual unknown classifier report](reports/2026-06-18-residual-unknown-classifier-report.md)
- [2026-06-15 fresh-crate control-plane validation](reports/2026-06-15-fresh-control-plane-validation.md)
- [2026-06-14 stance-change validation (#1705-1718)](reports/2026-06-14-stance-change-validation.md)
- [2026-06-13 fresh-crate capstone validation](reports/2026-06-13-fresh-crate-capstone-validation.md)
- [2026-06-13 post-fix card-correctness validation](reports/2026-06-13-post-fix-card-correctness-validation.md)
- [2026-05-26 post-burst analyzer snapshot](reports/2026-05-26-post-burst.md)
- [2026-05-26 arrayvec Vec::set_len rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md)
- [2026-05-26 crossbeam atomic pointer rerun](reports/2026-05-26-crossbeam-atomic-pointer-rerun.md)
- [2026-05-26 memchr unknown comment-plan follow-up](reports/2026-05-26-memchr-unknown-comment-plan.md)
- [2026-05-26 mio FFI route wording](reports/2026-05-26-mio-ffi-route-wording.md)
- [2026-05-26 no-card fixture smoke](reports/2026-05-26-no-card-control.md)
- [2026-05-27 arrayvec PR 138 UTF-8 follow-up](reports/2026-05-27-arrayvec-pr138-utf8-follow-up.md)
- [2026-05-27 hashbrown NonNull follow-up](reports/2026-05-27-hashbrown-nonnull-follow-up.md)
- [2026-05-28 memchr target-feature posture](reports/2026-05-28-memchr-target-feature-posture.md)
- [2026-05-28 arrayvec first-pr projection smoke](reports/2026-05-28-arrayvec-first-pr-projection-smoke.md)
- [2026-05-29 arrayvec Self::new capacity control](reports/2026-05-29-arrayvec-self-new-capacity-control.md)
- [2026-06-03 Bun manual candidates first-pr smoke](reports/2026-06-03-bun-manual-candidates-first-pr-smoke.md)

Report requirements:

- Every report must be linked from this README.
- Triage tables that include `Primary label` must use a label from
  [`triage-taxonomy.md`](triage-taxonomy.md).
- Follow-up seed rows must use known corpus targets, known operation-family or
  projection-surface labels, known triage labels, known statuses, and source
  reports linked from this directory. The linked source report must contain a
  triage row for the same target and primary label.
- Bun stable-byte follow-up seed rows must reference committed manual
  candidate examples, use known stable-byte families, preserve the candidate
  proof mode, use known ledger states, and use labels from
  [`stable-byte-triage-taxonomy.md`](stable-byte-triage-taxonomy.md).
- Reviewer judgments record manual usefulness samples for selected dogfood
  targets. They are measurement inputs only; they are not calibration,
  precision/recall, policy readiness, witness adequacy, or safety evidence.
  Committed judgment files must reference known targets, linked reports, known
  card families or review-kit surfaces, and the advisory trust boundary.
- External pilots record read-only public Action or equivalent first-pr bundle
  runs against real external PRs. They are product-usefulness evidence only:
  setup friction, runtime, artifact size, comment selection/omission, and human
  judgments. They do not authorize source edits, witness execution, comments,
  reviews, or issue filing in third-party repositories.
- Agent repair experiments measure whether one ReviewCard context packet and one
  repair-queue item produce a bounded, reviewable dry run. They are manual
  experiments only; `unsafe-review` does not run an agent, execute witnesses,
  edit source, post comments, suppress cards, resolve cards, or enforce policy.
- Every report must include a `## Trust boundary` section that names witness,
  safety, UB-free, Miri-clean, site-execution, calibration, and policy limits.

`cargo run --locked -p xtask -- check-dogfood` verifies these report rails.

## Fixture Controls

`fixture-control` targets are explicit false-positive controls that live under
`fixtures/`. They may exercise quiet/no-card behavior or other dogfood rails,
but they do not count as real-crate coverage or calibrated precision evidence.

## PR Diff Targets

`pr-diff` targets are repeatable only when the `root` checkout matches the
source tree expected by the saved diff. Do not record an exploratory historical
PR diff if it only produced zero cards because the local checkout had drifted
away from that PR's files or line ranges.

Record a zero-card PR diff only when the zero-card result is the intended
evidence, such as a false-positive control, and explain that in the target
`purpose`.

When an exploratory real PR exposes an unsupported class that produces zero
cards, record it as a named limitation in the dogfood handoff or objective audit
instead of counting it as an active corpus target. A zero-card result is not
evidence that the PR is safe.

When running a real PR from exact product input, fetch the base branch and PR
ref into a local checkout, keep the exact base/head SHAs visible, and run:

```bash
unsafe-review pr --base-sha <base-sha> --head-sha <head-sha>
```

The tool validates the checked-out head before analysis. If a dogfood target
needs a saved raw diff, use `git diff --output=<path>` so the saved file is not
shaped by the shell:

```bash
git -C target/dogfood-work/hashbrown diff --no-ext-diff --binary \
  --output=/absolute/path/to/target/dogfood-work/hashbrown-pr681.raw.diff \
  <base-sha> <head-sha>
```

If a dogfood run must stream `gh pr diff`, feed the stream directly to
`unsafe-review pr --diff -`; do not save product-input patches through
PowerShell redirection.
