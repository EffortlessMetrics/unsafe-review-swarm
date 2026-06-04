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
[`usefulness-notes.md`](usefulness-notes.md). Dogfood report labels are defined
in the [`triage taxonomy`](triage-taxonomy.md). Follow-up work seeds are tracked
in [`follow-up-seeds.md`](follow-up-seeds.md). Reviewer judgment files follow
the [`dogfood judgment schema`](judgments/README.md). Card-scoped agent repair
dry runs follow the [`agent repair experiment protocol`](agent-repair-experiments.md).

Snapshot reports:

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
- Reviewer judgments record manual usefulness samples for selected dogfood
  targets. They are measurement inputs only; they are not calibration,
  precision/recall, policy readiness, witness adequacy, or safety evidence.
  Committed judgment files must reference known targets, linked reports, known
  card families or review-kit surfaces, and the advisory trust boundary.
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

When capturing a raw GitHub PR diff, use `rtk proxy` so the saved file keeps the
full patch shape:

```bash
rtk proxy gh pr diff 681 -R rust-lang/hashbrown --patch \
  > target/dogfood-work/hashbrown-pr681.raw.diff
```
