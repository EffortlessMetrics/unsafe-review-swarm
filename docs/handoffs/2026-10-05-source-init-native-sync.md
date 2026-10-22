# 2026-10-05 - source init native sync

Scope: absorb the unpublished native init corrections from
[source PR #576](https://github.com/EffortlessMetrics/unsafe-review/pull/576)
under [issue #1885](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1885).
The checkpoint owner is `repo-infra`; product handoff behavior remains #1885.
The source-sync section of [SWARM_TO_MAIN](../contributing/SWARM_TO_MAIN.md)
covers this narrow behavior sync. The publication-metadata mirror runbook is
not the governing route for these unpublished fixes.

## Exact source and workbench identities

- Prior swarm main: `3ea96a4651123001c4753b15babb05e2653438c9`, carrying
  swarm PR #2350.
- Prior swarm source checkpoint: `f1eda818549d324d04071ffafa407fcc3c11da92`,
  retained in the [0.5.0 receipt handoff](2026-09-18-source-0.5.0-publication-sync.md).
- Source merge: `735fb7ba1107b79a9c18bd9b6bf169eeeae68cd1`, parents
  `f1eda818549d324d04071ffafa407fcc3c11da92` and
  `5cd2730877475ad8d8572f42d4a3ce6a38bf3e2e`.
- Source merge and reviewed-head tree:
  `954fba2d980ecd5b949b2713913511ac2e81ba75`.

The nine source commits since f1eda818 contain six final changed paths.
Temporary source CI observers have no final delta. Source's copied older
source-sync policy is deliberately not imported: doing so would regress
swarm's already-acknowledged 0.5.0 receipt.

## Absorption by path

| Source path | Swarm disposition |
| --- | --- |
| `crates/unsafe-review-cli/src/execute/init.rs` | Carry the exact final source blob `8e64ece2287dee1fc43a264001da193853d8bc61`: retain native canonical paths for inspection, require Windows command spelling to round-trip to that identity, and detect checkout presence through ASCII Git status. |
| `crates/unsafe-review-cli/tests/e2e.rs` | Carry the Windows ordinary/period/space assertions and Unix non-UTF-8 checkout-warning assertion into the newer swarm harness. Keep swarm-only help/scope/environment/agent tests and helpers; whole-file equality is deliberately not claimed. |
| `crates/unsafe-review/tests/init_consumer.rs` | Carry the exact source facade test blob `1e3af9e8e72753813304278cbe9a5a9d2e21d8bd`. It stages the source-built executable in a disposable PATH prefix and follows actual Unix generated commands; it does not install a package. |
| `docs/FIRST_USE.md` | Carry executable-discovery/recovery and native-path limitations, preserving development-checkout wording. |
| `docs/specs/UNSAFE-REVIEW-SPEC-0023-first-hour-experience.md` | Carry native inspection, ambiguous-command refusal and encoding-independent checkout-presence requirements. |
| `plans/work-specs/examples/UNSAFE-REVIEW-WORK-1885.toml` | Retain original root/base/preview acceptance, add source-native/facade acceptance and this bounded absorption/checkpoint contract. |

No source ancestry merge, whole-tree copy, dependency update, workflow/gate
change, unsafe allowance, toolchain/MSRV change or version selection is part of
this sync. Existing swarm features and publication receipts remain authoritative.

## Proof and limits

The [source native RED](https://github.com/EffortlessMetrics/unsafe-review/actions/runs/37253074390)
compiled the Linux and Windows targets, then failed the Unix non-UTF-8
checkout-warning case and Windows period/space cases (exit101, one test each).
The [source native GREEN](https://github.com/EffortlessMetrics/unsafe-review/actions/runs/37253388300)
passed all six Unix and four Windows named cases (exit0, one test each).
Production init and the carried native test bodies are identical to that
tested source; the full swarm e2e harness has a different blob.

The source exact-head [CI](https://github.com/EffortlessMetrics/unsafe-review/actions/runs/37253702917)
and [independent review](https://github.com/EffortlessMetrics/unsafe-review/pull/576#pullrequestreview-5409380801)
passed, as did source-main [post-merge CI](https://github.com/EffortlessMetrics/unsafe-review/actions/runs/37259353794).
These are retained source evidence, not new execution of the complete swarm
Windows suite.

This swarm PR requires its own current-head hosted core/Policy Contracts and
independent review. `source-divergence` should report
`new_source_commits=0` after the checkpoint advances to735fb; raw ancestry
differences remain expected. A matching GitHub source-main/checkpoint comparison
is source-object evidence, not execution of that Cargo command. Local Cargo
remains unrun under the source-only executor allocation. Record actual hosted
check and advisory-command results in the linked PR rather than treating
pending or skipped checks as passed.

## Next installed-consumer qualification

#1887 owns the installed editor/agent loop, #1881 the actual external-usefulness
denominator, and #1918 executable discovery/support. Their next candidate must
bind exact source SHA, lockfile and executable hash on the current1.98 baseline.
The historical #1921/#1925 receipts and hardcoded0.4.0 Windows harness do not
qualify this source merge. Use existing nonpublishing package-list/archive and
clean-prefix source-install paths; label source-built, archive-verified and
registry-resolved evidence separately. Current manifest identity0.3.8 alone
cannot distinguish the old installed binary from these repairs. No package,
tag, publication or installed-binary replacement is selected here.

## Trust boundary and rollback

This handoff establishes narrow source absorption and preserves existing
native-path proof. It makes no installed-package, broad PowerShell/UNC/long-path,
actual ub-review/cargo-allow/RIPR integration, witness, safety, UB-free,
Miri-clean, site-execution, calibrated-accuracy, policy-readiness or release
claim. Reverting this sync restores the prior behavior/checkpoint; the source
merge and previous publication receipts remain intact.
