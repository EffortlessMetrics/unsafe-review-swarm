# Dogfood report: 2026-06-19 generalization validation closeout

Status: GPR-5 validation closeout
Swarm base commit: `c42ad18e`
Artifact status: committed evidence index only; no new corpus run in this closeout
Refresh: 2026-06-22 external pilot receipt count updated after #1832

This report closes the first post-0.3.8 generalization slice. It summarizes the
checked rails that now exist for corpus partitions, holdout execution,
evidence-loss challenges, external pilot receipts, and cross-surface consumer
contracts.

The closeout is intentionally narrow. It does not add a detector, widen comment
posting, promote a holdout case to regression, or claim that one pilot proves
general external usefulness. It records what the current evidence can and cannot
support, then names the next lane.

## Trust boundary

This is static unsafe contract review product evidence only. It is not a
calibration report, not precision or recall, not a benchmark, not a release
claim, not memory-safety proof, not UB-free status, not Miri-clean status, not a
witness result, not site-execution proof, not policy readiness, and not a merge
verdict. No witness tools were run for this closeout. External pilot evidence is
read-only product-usefulness evidence, not third-party review authority.

## Evidence Sources

| Layer | Current source of truth | Checked by | What current evidence supports |
|---|---|---|---|
| Conformance partitions | `fixtures/calibration.toml`, `policy/pr-corpus.toml`, `policy/evidence-loss-challenges.toml` | `xtask check-corpus-partitions` | Corpus cases resolve to one partition owner, and holdout cases cannot silently enter every-PR cadence. |
| Regression corpus | `docs/dogfood/corpus.toml`, release-readiness docs | `xtask check-dogfood`; release-only `dogfood-exec --strict` | Pinned real-repo and real-PR sources are governed as diagnostics. Heavy execution remains release/nightly, not every PR. |
| Holdout | `docs/dogfood/corpus.toml`, `docs/dogfood/reports/2026-06-19-initial-holdout-report.md` | `dogfood-exec --include-holdout` plus default negative opt-in check | `getrandom-holdout` has an exact SHA, a first capped result before tuning, and explicit opt-in execution. |
| Evidence-loss challenges | `policy/evidence-loss-challenges.toml` | `xtask check-evidence-loss-challenges` | The first controlled evidence-loss transformation regresses the expected ReviewCard movement and preserves low-noise surfacing. |
| External pilots | `docs/dogfood/pilots/` | `xtask check-external-pilots` | Four read-only local-equivalent pilot receipts now record setup friction, runtime, artifact size, selected and omitted comments, and usefulness judgments across `bytes#827`, `getrandom#811`, `memchr#226`, and `hashbrown#692`. |
| Cross-surface contract | `docs/specs/UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md`, `docs/specs/UNSAFE-REVIEW-SPEC-0034-ub-review-gate-manifest.md` | `xtask check-first-pr-artifacts` | `unsafe-review-gate.json` is now a checked first-pr consumer contract with fixed advisory status, movement projection, artifact pointers, and exact trust-boundary wording. |

## What Generalized

- The corpus system now has explicit conformance, regression, and holdout
  ownership without creating a duplicate corpus ledger.
- The holdout path is opt-in by construction: a targeted holdout run without
  `--include-holdout` fails before cloning or scanning.
- Known evidence-loss transformations are now a first-class check shape. The
  initial challenge shows the harness can detect a named contract-evidence
  regression on a realistic fixture input.
- External pilots now have a receipt schema and checker, so usefulness evidence
  can be recorded without third-party writes or accuracy claims. The first four
  receipts include `getrandom#811` for an FFI boundary, `memchr#226` for pointer
  arithmetic, `hashbrown#692` for raw-pointer write, pointer arithmetic, and
  slice-from-raw-parts seams together, and `bytes#827` as a low-noise
  comment-selection/setup-friction pilot rather than a one-seam classifier
  sample. All preserve read-only operation.
- First-pr consumer surfaces have a stronger contract rail: changing the gate
  manifest trust boundary, movement counts, status, or artifact pointers now
  fails the verifier.

## What Failed Or Remains Thin

- The holdout set has one recorded target. That is enough to establish the
  holdout mechanism, not enough to claim ecosystem generalization.
- The external pilot set has four recorded PRs. That is enough to exercise the
  receipt rail across more than one project and card shape, but still not enough
  to characterize maintainer usefulness across projects.
- The evidence-loss challenge ledger has one transformation. It establishes the
  challenge harness and one evidence-loss class, not global recall.
- The real-PR corpus is still mostly synthetic movement fixtures. Real external
  PRs should stay in pilot receipts until a release-readiness decision promotes
  a case into `policy/pr-corpus.toml`.
- The residual unknown report still recommends no classifier PR. Unsafe impls
  are a real bucket, but the stance/corpus evidence for that lane is separate
  and not established by this closeout.

## Noise And Friction

The main actionable friction is first-use input acquisition, not detector
breadth. The `bytes#827` pilot needed an explicit raw diff path over exact SHAs;
PowerShell redirection and default `gh diff` output were not the effortless path
a new maintainer should need. The later `hashbrown#692` pilot showed a related
historical-PR wrinkle: the GitHub-reported base ref was gone, but the immutable
base SHA was still reachable, so the receipt had to rely on SHA-pinned checkout
and diff generation.

That friction is product evidence, but it should not be fixed by widening this
closeout. The next PR should target a small first-use improvement that makes
raw PR diff acquisition or public-Action pilot setup harder to get wrong, while
preserving read-only defaults and the advisory boundary.

## Next Lane Recommendation

Start a first-use friction PR before another analyzer-family PR:

```text
problem:
  external pilot setup still requires too much judgment around raw PR diff
  acquisition and exact base/head inputs

target seam:
  first-use docs or a small read-only helper that records/copies the exact
  command needed to produce the first-pr diff input

acceptance:
  - no source edits
  - no third-party comments or issues
  - no witness execution
  - exact base/head SHAs stay visible
  - generated bundle remains the normal first-pr bundle
  - pilot receipts can cite the simpler path

non-goals:
  - no default posting
  - no default blocking
  - no semantic analysis
  - no unsafe impl classifier
  - no accuracy or recall claim
```

## Closeout Decision

GPR-5 is complete for the initial generalization rail: the repo now has checked
mechanisms for partitioning, holdout opt-in, evidence-loss challenges, external
pilot receipts, and gate-manifest consumer contracts, plus an explicit record of
what remains thin.

Do not promote this to a broad product-validation claim. The next product work
should make external first-use easier, then add more pilots and holdout samples
before selecting another analyzer family.
