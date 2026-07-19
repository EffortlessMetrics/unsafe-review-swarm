# Dogfood report: 2026-07-19 holdout contract audit

Status: docs/governance audit (issue #1891, PR 1)
Swarm base commit: `18e33cb9`
Artifact status: docs-only audit; no `dogfood-exec` run and no new corpus target in this report

This report is the first of a two-or-more-PR arc on holdout governance
(issue #1891). It audits what already exists for the dogfood holdout
partition — inventory, opt-in enforcement, and tuning-input status — and
records rotation, first-result retention, promotion-to-regression, and
target-replacement rules **by reference** to
`docs/specs/UNSAFE-REVIEW-SPEC-0042-corpus-validation-taxonomy.md`, which
already defines them. It adds no holdout target, no duplicate corpus
manifest, and no analyzer or detector code change. Adding new holdout
targets is out of scope for this PR and is left to PR2+.

## Trust boundary

This is static unsafe contract review advisory evidence only. It is not a
memory-safety proof, not a safe/UB-free status claim, not Miri-clean status,
not a witness result, not site-execution proof, not calibrated precision or
recall, and not a policy or release-readiness decision. No witness tools were
run for this audit. The holdout result referenced below is diagnostic
evidence on one selected pinned repository, not ecosystem generalization
evidence.

## Scope

- Audit target: `docs/dogfood/corpus.toml` holdout partition membership.
- Audit target: `xtask/src/dogfood_exec.rs` and `xtask/src/corpus_partitions.rs`
  opt-in enforcement rails.
- Audit target: `git log` history for any analyzer/detector/ranking change
  motivated by `getrandom`.
- Reference-only: `docs/specs/UNSAFE-REVIEW-SPEC-0042-corpus-validation-taxonomy.md`
  Refresh policy and Generalization partitions sections.
- Out of scope: adding, rotating, or promoting any holdout target; that is
  PR2+ work and is explicitly named as backlog below, not started here.

## 1. Holdout inventory

The only current holdout target is `getrandom-holdout`, declared in
`docs/dogfood/corpus.toml`. Quoting the manifest entry as of this audit:

```toml
[[targets]]
id = "getrandom-holdout"
crate = "getrandom"
repository = "rust-random/getrandom"
kind = "repo-snapshot"
partition = "holdout"
status = "active"
commit = "5e7cd5733536844a9856dc7259bd4696bbe5e3ae"
root = "target/dogfood-work/getrandom-holdout"
purpose = "release-readiness holdout repo snapshot for platform RNG unsafe declarations, FFI-style bindings, and raw pointer cards; first result is recorded before tuning"
command = "rtk cargo run --locked -p unsafe-review -- repo --root target/dogfood-work/getrandom-holdout --format json --max-cards 50 --out target/dogfood-work/getrandom-holdout.unsafe-review.json"
artifact_status = "local_untracked"
artifacts = ["target/dogfood-work/getrandom-holdout.unsafe-review.json"]
```

`docs/dogfood/index.json` records `summary.partitions.holdout: 1`, matching
this single-target inventory. This PR does not add a target, so that count
stays `1`.

The first (and so far only) execution of this target is recorded in
`docs/dogfood/reports/2026-06-19-initial-holdout-report.md`: a capped
`repo-snapshot` run pinned to commit `5e7cd5733536844a9856dc7259bd4696bbe5e3ae`,
producing 50 cards (50 summary cards, 139 unsafe sites, 41 Rust files, 2.0%
unknown, 8.0% target-feature) with status `ok`. That report explicitly
records the result as a first diagnostic reading, not a tuning target, and
directs any analyzer change motivated by the target through a follow-up
report or issue rather than a direct edit.

Separately, `docs/dogfood/pilots/` records a read-only external pilot receipt
for `getrandom#811` (an FFI-boundary PR). That pilot receipt is a distinct
evidence class from the `getrandom-holdout` repo-snapshot target: it is
product-usefulness pilot evidence under the external-pilot rail
(`xtask check-external-pilots`), not a holdout execution, and it does not
change the holdout inventory or its tuning-input status recorded below.

## 2. Opt-in enforcement evidence

A holdout target cannot silently enter every-PR or ordinary regression
cadence. Two independent rails enforce this:

- **`xtask/src/dogfood_exec.rs`, `targets_for_args` (function starts at line
  396; the holdout-skip branch is at roughly lines 416-436).** When
  `--include-holdout` is not passed, a targeted run naming a holdout target
  by `--target` fails with an explicit error (`` `{id}` is partitioned as
  holdout; rerun with --include-holdout... ``) before cloning or scanning,
  and an untargeted run silently drops holdout targets from the batch while
  printing a skip notice. Holdout execution is opt-in by construction, not by
  convention.
- **`xtask/src/corpus_partitions.rs`, `reject_every_pr_holdout` (line 320).**
  This structural check rejects any holdout-partitioned entry in any of the
  governed manifests (`fixtures/calibration.toml`, `docs/dogfood/corpus.toml`,
  `policy/pr-corpus.toml`, `policy/evidence-loss-challenges.toml`) that sets a
  `cadence`, `run_cadence`, or `tuning_cadence` field to `"every-pr"`. This is
  a manifest-level backstop independent of the execution-time gate above: even
  a manifest edit that tried to declare every-PR cadence for a holdout case
  would fail `check-corpus-partitions`.

Both rails are exercised by targeted tests in `xtask` (the dogfood-exec
negative opt-in path and the corpus-partitions holdout-cadence rejection
path) and by the negative command recorded in the initial holdout report:
`dogfood-exec --target getrandom-holdout --strict --timeout 30` (no
`--include-holdout`) fails before cloning or scanning with `rerun with
--include-holdout`. Together, these two mechanisms are the audit evidence
that `getrandom-holdout` cannot silently migrate into every-PR or ordinary
regression execution.

## 3. Tuning-input check

Question: has `getrandom-holdout` already become a tuning input — i.e., has
any analyzer, detector, or ranking change been made because of what this
holdout produced?

Method: `git log --all -S "getrandom"` and `git log --all -S
"getrandom-holdout"` over the full swarm history, followed by manual
inspection of every matching commit's diff.

Finding: **not tuned against.** Five commits match a `getrandom` string
search, and none is an analyzer, detector, or ranking change motivated by the
`getrandom-holdout` target:

| Commit | Subject | Why it matches | Analyzer/detector change? |
|---|---|---|---|
| `26e2541f` | docs(ci): document source-divergence local-only posture in SPEC-0024 §4.6 (#1809) (#1827) | Squash-merge commit that introduced the `getrandom-holdout` corpus entry and its first-result report | No — adds the target and the docs artifacts; no `crates/unsafe-review-core` or `xtask` analyzer/detector diff |
| `9032f112` | docs(corpus): refresh external pilot closeouts (#1833) | Mentions `getrandom-holdout` and the unrelated `getrandom#811` pilot receipt in a table | No — docs-only |
| `e254bade` | test(fuzz): add analyzer harness (#285) | `Cargo.lock` entry for the `getrandom` crate as a transitive dependency of fuzz tooling | No — unrelated crate-name collision, not the holdout target |
| `0cd47e8b` | test: add core property invariants | Same `Cargo.lock` transitive-dependency collision | No |
| `4c3d0709` | seed: import public unsafe-review main for swarm repo | Same `Cargo.lock` transitive-dependency collision | No |

The three `Cargo.lock` matches are the `getrandom` crate appearing as an
ordinary transitive dependency of test/fuzz tooling — an unrelated name
collision with the analyzed target crate, not evidence of tuning. No commit
in the history touches `crates/unsafe-review-core/src/analysis/`,
`crates/unsafe-review-core/src/domain/`, or any ranking/classification module
in a way connected to `getrandom` or the holdout result. This matches the
expected finding recorded in the initial holdout report: the first result was
recorded before tuning, and it remains untouched by any subsequent analyzer
change as of this audit.

## 4. Rotation, retention, promotion, and replacement (by reference)

These rules already exist in
`docs/specs/UNSAFE-REVIEW-SPEC-0042-corpus-validation-taxonomy.md` under
**§Refresh policy** (governs all partitions) and **§Generalization
partitions** (defines the holdout partition specifically). This report does
not restate or duplicate that text; it summarizes operationally what already
applies to `getrandom-holdout` and to any future holdout target:

- **Rotation cadence**: rotate part of the holdout set each release cycle
  when fresh suitable inputs are available (§Refresh policy). No cadence
  timer exists outside that release-cycle framing; rotation is a
  release-readiness decision, not a scheduled job.
- **First-result retention**: retain historic snapshots or reports for trend
  comparison (§Refresh policy); the `2026-06-19-initial-holdout-report.md`
  first-result report stays in place as the retained baseline even after any
  future re-run.
- **Promotion-to-regression criteria**: promote a holdout case into
  regression only after the initial result and a follow-up decision are
  recorded (§Refresh policy). Mechanically, promotion for a `repo-snapshot`
  target is removing its per-target `partition = "holdout"` override, since
  `repo-snapshot` defaults to `regression` via `partition_by_kind`
  (§Generalization partitions). No such follow-up decision has been recorded
  for `getrandom-holdout`; it remains in the holdout partition per the
  promotion path already named in the initial holdout report.
- **Target-replacement rules**: do not tune directly against holdout
  findings before recording the result (§Refresh policy); a target may be
  rotated out of holdout and replaced with another fresh pinned target
  instead of being promoted, per the same promotion-path note in the initial
  holdout report. Every partition stays advisory: no precision, recall,
  UB-free, Miri-clean, site-execution, or memory-safety proof claim is
  created by partitioning or by a promotion/rotation decision
  (§Refresh policy).

No holdout rotation, promotion, or replacement decision is made in this PR.
This section only records where the governing rules live and how they apply
to the current single-target inventory.

## 5. Unmet dimension matrix (PR2+ backlog)

`getrandom-holdout` exercises platform RNG unsafe declarations, FFI-style
bindings, and raw pointer operations, so it partially covers the FFI/platform
dimension. The dimensions below remain unrepresented, or only incidentally
touched, in the current one-target holdout set. This is a named backlog for
PR2+, not a selection made in this PR:

| Dimension | Current holdout coverage | Note |
|---|---|---|
| FFI/platform bindings | Partial (`getrandom-holdout`) | Covered incidentally by the RNG/platform surface; not exercised as a dedicated FFI-boundary target |
| Raw pointers / alloc / `MaybeUninit` | None dedicated | Present in regression targets (e.g. `arrayvec-capped`, `bumpalo-capped`), not in holdout |
| SIMD / `target_feature` / inline asm | None | No holdout target exercises SIMD intrinsics or inline asm surfaces |
| Atomics / concurrency / lock-free structures | None | No holdout target exercises atomics or lock-free data structures |
| `no_std` / embedded | None | No holdout target is a `no_std` or embedded-oriented crate |
| Macro-heavy / generated code | None | No holdout target exercises heavy declarative or procedural macro generation |
| Multi-crate workspace | None | `getrandom-holdout` and all regression targets are single-crate snapshots |
| Quiet / small unsafe surface | None | No holdout target is selected specifically for a small or quiet unsafe footprint |

Selection for any future holdout addition must not be "produces many cards."
A target that maximizes card count optimizes for the wrong signal; holdout
selection should target dimension coverage (the gaps above) and realistic,
previously-unseen code shape, consistent with the fixture-suite-blindness
rationale already recorded in this repository's `CLAUDE.md` and in SPEC-0042
Layer 3/4 "what it is blind to" sections. PR2+ should pick from the unmet
dimensions above rather than optimizing for scan output volume.

## Decision

This PR records the audit only: current inventory (one target,
`getrandom-holdout`), two independent opt-in enforcement rails, a confirmed
not-tuned-against finding, rotation/retention/promotion/replacement rules
cited by reference to SPEC-0042, and an explicit unmet-dimension backlog. It
adds no holdout target, no duplicate manifest, and no analyzer change. PR2+
should select and add a holdout target from the unmet-dimension backlog above
under the rules referenced in §4, and should record that addition as its own
first-result report following the same shape as
`2026-06-19-initial-holdout-report.md`.
