# Installed consumer qualification — 2026-10-05

[PR #2352](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2352)
delivers the bounded [#1887](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1887)
qualification slice, the bounded stale verifier repair under
[#2302](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2302#issuecomment-5988788206), and [#1918](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1918)
support evidence under [WORK-1887](../../plans/work-specs/examples/UNSAFE-REVIEW-WORK-1887.toml).
These umbrella issues remain open.

## Identity and execution class

- Exact source candidate: `735fb7ba1107b79a9c18bd9b6bf169eeeae68cd1`.
- Source tree: `954fba2d980ecd5b949b2713913511ac2e81ba75`.
- Tracked Cargo.lock SHA-256: `0b3e7d6b5f06e6316125c1dffb89e353cc1dfde1fe2632ee09584863777863fd`.
- All three candidate package manifests and the selected executable report **0.5.0**.
  This corrects the earlier inference from Swarm's development 0.3.8 identity.
  The regression fixture's 0.3.8 is a test input, not the actual candidate version.
- Existing repository baseline: Rust 1.98; actual hosted compiler: rustc 1.98.1.
- Install class: `cargo install --path crates/unsafe-review --root <clean-prefix> --locked --jobs 1`
  in an owned hosted scratch directory.
- `cargo package --locked --list` inventories all three packages; it does not
  install a package archive. Registry installation and archive installation are **NOT_RUN**.
- Windows manual dispatch/runtime is **NOT_RUN**. This observation is hosted Linux.

## Executed proof

Tests-first receipt [RED](https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/37263286018)
at `3b7c0e5c2eb0361f9f6e1d091d7a9573d41db0a4` failed the actual
hardcoded-version and manifest/executable-mismatch assertions.
The unchanged real Source bundle fixture then exposed stale cue verification in
[RED 37266807557](https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/37266807557),
selected-body prefix in [partial 37269507336](https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/37269507336),
and the actual legacy witness surface in [partial 37270729585](https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/37270729585).
Those partial runs passed consumer subsets but correctly retained aggregate FAIL.

[GREEN 37271592490/job 111639570647](https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/37271592490/job/111639570647)
at `364554f69d51dbbeb1f94c3bc16c546a00aa86ef` records:
**core exit 0; seven verifier controls PASS; three receipt tests PASS; all 19
qualification rows PASS; actual Source bundle exit/verifier exit 0; all 20 consumer
assertions PASS; aggregate `QUALIFICATION_SOURCE_PREFIX_COMPLETE`.**
[Policy Contracts 37271592603](https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/37271592603)
also passed. Advisory step success alone is not the qualification verdict.

- Selected executable SHA-256: `cd54a30bb687a45d32630fa81200360088c7744e8fcdf1bd4d49365ccb3a33b0`.
- Actual probe SHA-256: `97b0b9103a6c3ef59acf9e86d90e3cb000aafe7b6d58ac5f30ba1d565fb3a4cf`,
  identical to the final local legacy-control probe.
- Computed qualification receipt SHA-256: `ec15e02a9028a47d30c04b92b5faad2cf77d8792f1c797b598363f5ec9ac572b`.
- Verifier Rust blob: `897171c06d81913ace8def3d26649d09b8e17e82`; retained unchanged when removing the observer.
- Normal bounded artifact [11329210884](https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/37271592490/artifacts/11329210884):
  4,732 bytes, metadata digest `ee74da759a2a7d7b115f747f2b2223c5e3b0bac49fa6455874bcd2a5fb6aa03b`.
  Its contents were not locally downloaded; computed proof is in its own normal job logs.

| Package inventory | Files | List SHA-256 |
| --- | ---: | --- |
| unsafe-review-core | 164 | `7fb8279b7c545f81953459674f3e70badadfbdacada7f3ea454664d92a3e4952` |
| unsafe-review-cli | 33 | `f80470990ccf87507d0d0023ebf697649dffa3b8eb6a66dc8fbc79505888d3be` |
| unsafe-review | 10 | `b5977ca35bbcfa09027ef06f090f717004fb5d09663fc6ac94f1de8988b82581` |

Final exact-head independent review, current required gates and protected merge
are recorded in the PR and canonical issue closeout. They remain distinct from
the immutable qualification run above.

The unchanged real-code bundle fixture changes `pub fn f() -> i32 { 1 }`
to `pub fn f() -> i32 { unsafe { core::mem::zeroed() } }` without adding a
reaching test. Its old verifier rejection is a real RED, not a compile failure.
Intermediate proof passed focused controls and all 20 consumer rows but exposed
additional selected-body and witness-surface assumptions; those are retained as
partial FAIL evidence.
The repair matches all three canonical test-first cue projections using the
existing exact reach-summary parser, with owner/obligation consistency checks.
Reached, ownerless, receipt-backed and commandless posture remains distinct;
missing/stale/forged cues cannot bypass command, subject or trust-boundary checks.
The labeled comment step must contain the full canonical step; canonical text
elsewhere cannot rescue a forged label. The actual current witness writer uses
distinct legacy prose, which is checked only on that surface. Actual renderer
controls exercise an unreached function and then a reaching test.
[Witness-prose alignment remains an explicit #2302 follow-up](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2302#issuecomment-5989163965);
canonical JSON/comments/PR summaries do not accept its legacy cue.
This aligns artifact projection integrity; it does not grant route readiness
or permission to execute.

The consumer assertions require init-specific first-line help, actual preview
schema and no repository writes, explicit null/missing-base recovery, an
explicit proposal envelope, target-local artifact destinations, literal shell
arguments, unique canonical doctor root, changed-seam-only cards, a freshly
written third-cwd replay, preserved owner workflow, a safe-only no-sites control,
and caller/third-cwd output confinement. Only exercised cases are qualified.

The receipt verifies consistency with the supplied source-install receipt;
it is not an independent build attestation. It records executable, source,
tracked/working lockfile and package-list identities. Row elapsed time is
recorded; representative CPU time and peak memory are not measured.

## Unchanged installation control

The pre-existing 5700X facade remains untouched:
the user's existing Cargo-bin `unsafe-review.exe`, SHA-256
`e8202f8b3ea24341e41b0b18a31f57b3d0484884ae8c43c38eb3a020df62743e`.
The final probe logic reports 0.3.8, rejects exit-zero global help, observes
actual init exit 2, and confirms preview control files unchanged. It stops
before later consumer rows and reports `not_qualified`, with unknown source
binding. Historical receipts were preserved.

## Boundaries and next owners

The candidate CLI is unchanged Source 735fb; the new verifier and qualification
harness are development Swarm work. Source's older verifier/harness is not
silently replaced or qualified by this result. Curated tooling promotion remains
with repo-infra's existing source-promotion/source-sync ownership.

The repaired manual Windows workflow retains its existing trigger, runner,
permissions, toolchain and actions. The temporary normal-PR observer is removed
before final review and merge. The workflow ledger change is descriptive only;
unsafe allowances, runner permissions and required-check configuration are unchanged.

The normal GH CLI workflow-metadata read failed before GitHub while opening
the user's existing GitHub CLI configuration file: `Access is denied.`
No manual dispatch, config read, retry, alternate credential or transport followed.
Normal artifact materialization for run 37264284861, artifact 11326047347,
file `file_00000000f78881f58d680b0addda52d0`, failed:
`cannot create attachment directory: Access is denied. (os error 5)`.
A separate raw job-metadata GET to
`https://api.github.com/repos/EffortlessMetrics/unsafe-review-swarm/actions/jobs/111617802097`
was rejected with HTTP 400: `GitHub Fetch URL is not an allowed public GitHub repository or search endpoint.`
It was not retried or rerouted. The exact actions/errors are retained; that attachment was not retried or retrieved
another way. Later proof establishes its own bounded diagnostics in normal job logs.

No broad Windows/PowerShell/UNC/long-path, installed LSP/editor session, actual
tokmd/ub-review/cargo-allow/RIPR provider, witness/site execution, calibrated
accuracy, distribution or release-readiness claim follows from this slice.
Malformed/non-UTF-8 doctor output fails qualification; no lossy root identity
fallback is introduced.

Continue the existing [incremental roadmap](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2223#issuecomment-5984305987):
#1887 owns the thin editor/agent loop; #1881 owns representative external usefulness
and performance; #1918 owns truthful support/availability; source #552–#557 owns
the actual provider path. Keep ub-review orchestration separate from cargo-allow
permission authority and preserve the restrictive unsafe policy.
Publication and version selection remain separately owned.

Rollback is a normal revert of this bounded harness/probe/documentation slice.
Local Cargo remains unadmitted; task-owned source/evidence remains below 128 MiB.
Other workers and unique evidence are preserved.
