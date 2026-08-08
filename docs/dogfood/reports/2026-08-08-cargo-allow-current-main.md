# Dogfood report: 2026-08-08 cargo-allow current-main reproduction

Status: issue #1890 evidence receipt, pre-release and advisory

This report reruns the cargo-allow report described by public issue
`EffortlessMetrics/unsafe-review#541` against one clean, pinned checkout. It
separates the latest published binary from the current development binary and
classifies every current-main actionable card. The checkout was not modified;
all generated bundles remain local and untracked.

## Trust boundary

This is static unsafe-contract review evidence for one pinned repository. It is
not memory-safety proof, UB-free status, Miri-clean status, site-execution
evidence, calibrated precision or recall, or policy-readiness evidence. No
witness ran, no source file changed, and no third-party issue or pull request
was modified by the scan.

## Inputs and runtime

| Input | Value |
|---|---|
| Repository | `EffortlessMetrics/cargo-allow` |
| Pinned input commit | `8d85d5b261762f83c3dbe8c184d6403aac95603e` |
| Checkout state | clean; Linux workspace; `cargo metadata` available |
| Published binary | `unsafe-review 0.3.1` |
| Current development binary | `unsafe-review 0.3.8` from swarm main `cf74f49778b23bb728541ac7772512edeb1f07d6` |
| Current source-divergence | `new_source_commits=0` with the checked-out swarm source-sync ledger |
| Witness state | no Miri signal; no witness execution requested or performed |

The published comparison was run from a separate clean checkout of the same
commit. The current-main checkout recorded `dirty_worktree=false` in its repo
report; the published 0.3.1 report predates that provenance field.

## Reproduction matrix

Both binaries ran the requested surfaces:

```text
unsafe-review doctor --root .
unsafe-review repo --root . --format json --out target/unsafe-review/reports/repo.json
unsafe-review badges --root . --out target/unsafe-review/reports/badges.json
unsafe-review policy report --root . --format json --out target/unsafe-review/reports/policy-report.json
unsafe-review receipt validate --root .
unsafe-review receipt audit --root . --format json --out target/unsafe-review/reports/receipt-audit.json
```

The current-main commands completed successfully. `receipt validate` reported
`0 valid` receipts. The published commands also completed successfully and
reported `0 valid` receipts.

| Surface | Published 0.3.1 | Current main 0.3.8 | Interpretation |
|---|---:|---:|---|
| Rust files | 1,029 | 1,029 | same checkout inventory |
| Unsafe sites | 13 | 13 | same detected sites |
| Actionable cards | 13 | 13 | no count movement |
| Contract missing | 12 | 12 | same semantic classification |
| Guard missing | 1 | 1 | same semantic classification |
| Receipt count | 0 | 0 | no receipt evidence present |
| Repo report schema | `0.1` | `0.2` | current main adds provenance/status metadata |

Current-main summary also reported `scan_capped=false`, `worsened_gaps=0`,
`resolved_gaps=0`, and `inherited_gaps=0`. The published report has the same
semantic counts but does not emit the newer movement/provenance fields.

## Artifact receipt

Generated artifacts are local-only. Hashes and byte sizes preserve reproducible
identity without committing third-party output.

| Binary | Artifact | Bytes | SHA-256 |
|---|---|---:|---|
| 0.3.8 | `repo-current-main.json` | 109,796 | `dc88e02cb1b57480f2d75cf42ae4c5b7097b24969309c9092b7027667b1ef8df` |
| 0.3.8 | `repo-current-main.json.status.json` | 1,276 | `c70ce6d847549e8ab3f1f8f242a4388622bee299389831656ce63164480a6ac0` |
| 0.3.8 | `unsafe-review.json` badge | 93 | `a4500b9e7c18c49ccc78dcba2713e2637dd84ada405065eaec38bcf35e0e89e8` |
| 0.3.8 | `unsafe-review-plus.json` badge | 94 | `947a0b1e840a3e407da4bcb43cbe8d7c3bb5c04673ddcd637562b9f8faf69f9e` |
| 0.3.8 | `policy-report-current-main.json` | 11,515 | `89d066c010110de9ea9789ac8b84a76a6cf2e2fb08321e9fa5c1ef1f166eb7ce` |
| 0.3.8 | `receipt-audit-current-main.json` | 1,409 | `fff92bee9c192d3356733beff3b888c7d8c92ec4bd7f6d0b88d008a0705db20e` |
| 0.3.8 | `unsafe-review-gate.json` | 470 | `0ec709cd7b674e368f822eeef7ea6a665ecce7b39191dfbdeadeab5b35b3a655` |
| 0.3.1 | `repo-published.json` | 78,085 | `5ed866f3b86f279c10e80a2d10ccfeb79ba5cec190d01baa69dab4a219b769ac` |
| 0.3.1 | `unsafe-review.json` badge | 628 | `49a7e3235db7bf2d86e708505455b8b26b29204f0e24ebd7a0635e076e8e819c` |
| 0.3.1 | `unsafe-review-plus.json` badge | 667 | `44bd36897bcbee9795a3e2a4518d05c86a92651546d42045e51b1ab8fcaa6659` |
| 0.3.1 | `policy-report-published.json` | 10,002 | `f90f3abc928daa706a631a8bef30fdf0e0fcdcdbe589a88b69e544e07be80cde` |
| 0.3.1 | `receipt-audit-published.json` | 980 | `5a0b8900cf52b55ad7dbe1ffdd7e4dd29f26e21f5dc872203b9d444b3929fc62` |

## Current-main card classification

The current-main report contains 13 actionable cards. The IDs below are the
unmodified ReviewCard IDs emitted by the stable ID generator.

| Card ID | Source range | Family | Primary classification | Reason and follow-up posture |
|---|---|---|---|---|
| `UR-cargo-allow-1890-crates-cargo-allow-src-cli-rs-run-operation-unsafe_fn_call-set-var-b31c4cebfdaf-unknown-c1` | `crates/cargo-allow/src/cli.rs:148` | `unsafe_fn_call` | `real_production_gap` | Production `std::env::set_var` call; a real safety-contract/guard review item. Route any fix as a one-seam production contract issue. |
| `UR-cargo-allow-1890-fixtures-unsafe-src-lib-rs-read-byte-unsafe_fn-unsafe_declaration-read-byte-c9aef8a4d43d-unknown-c1` | `fixtures/unsafe/src/lib.rs:1` | `unsafe_declaration` | `test_or_fixture_scope` | Deliberately unsafe fixture API used as a control; not production evidence. Existing fixture is the focused regression/control. |
| `UR-cargo-allow-1890-fixtures-unsafe-src-lib-rs-read-byte-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c1` | `fixtures/unsafe/src/lib.rs:2` | `raw_pointer_read` | `test_or_fixture_scope` | Deliberately unsafe fixture operation; keep as control evidence, not a product defect. |
| `UR-cargo-allow-1890-tests-fixtures-structural-identity-container-same-name-sibling-modules-after-rs-access-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c1` | `tests/fixtures/structural-identity/container_same_name_sibling_modules/after.rs:3` | `raw_pointer_read` | `test_or_fixture_scope` | Structural-identity fixture pair; the duplicate-looking cards are the test's intended identity signal. |
| `UR-cargo-allow-1890-tests-fixtures-structural-identity-container-same-name-sibling-modules-after-rs-access-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c2` | `tests/fixtures/structural-identity/container_same_name_sibling_modules/after.rs:8` | `raw_pointer_read` | `test_or_fixture_scope` | Structural-identity fixture pair; no production follow-up. |
| `UR-cargo-allow-1890-tests-fixtures-structural-identity-container-same-name-sibling-modules-before-rs-access-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c1` | `tests/fixtures/structural-identity/container_same_name_sibling_modules/before.rs:3` | `raw_pointer_read` | `test_or_fixture_scope` | Structural-identity fixture pair; no production follow-up. |
| `UR-cargo-allow-1890-tests-fixtures-structural-identity-container-same-name-sibling-modules-before-rs-access-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c2` | `tests/fixtures/structural-identity/container_same_name_sibling_modules/before.rs:8` | `raw_pointer_read` | `test_or_fixture_scope` | Structural-identity fixture pair; no production follow-up. |
| `UR-cargo-allow-1890-tests-fixtures-structural-identity-function-move-after-rs-read-right-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c1` | `tests/fixtures/structural-identity/function_move/after.rs:2` | `raw_pointer_read` | `test_or_fixture_scope` | Function-move fixture pair; existing fixture covers the intended identity behavior. |
| `UR-cargo-allow-1890-tests-fixtures-structural-identity-function-move-after-rs-read-left-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c1` | `tests/fixtures/structural-identity/function_move/after.rs:6` | `raw_pointer_read` | `test_or_fixture_scope` | Function-move fixture pair; no production follow-up. |
| `UR-cargo-allow-1890-tests-fixtures-structural-identity-function-move-before-rs-read-left-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c1` | `tests/fixtures/structural-identity/function_move/before.rs:2` | `raw_pointer_read` | `test_or_fixture_scope` | Function-move fixture pair; no production follow-up. |
| `UR-cargo-allow-1890-tests-fixtures-structural-identity-function-move-before-rs-read-right-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c1` | `tests/fixtures/structural-identity/function_move/before.rs:6` | `raw_pointer_read` | `test_or_fixture_scope` | Function-move fixture pair; no production follow-up. |
| `UR-cargo-allow-1890-tests-fixtures-structural-identity-module-move-after-rs-access-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c1` | `tests/fixtures/structural-identity/module_move/after.rs:2` | `raw_pointer_read` | `test_or_fixture_scope` | Module-move fixture pair; existing fixture covers the intended identity behavior. |
| `UR-cargo-allow-1890-tests-fixtures-structural-identity-module-move-before-rs-access-operation-raw_pointer_read-read-96b6e660e28d-pointer_validity-c1` | `tests/fixtures/structural-identity/module_move/before.rs:3` | `raw_pointer_read` | `test_or_fixture_scope` | Module-move fixture pair; no production follow-up. |

The 12 fixture-scoped cards are not detector failures merely because a full
repo scan includes them. They identify a future presentation/scope decision:
whether repo-scan consumers want an explicit test/fixture view or a separate
production-only view. That is a surfacing/product issue, not permission to
suppress all tests or fixtures and not a basis for a global accuracy claim.

## Disposition

- The historical 50-card result does not reproduce on current main: this
  pinned run produces 13 cards with the same 13-card semantic result under both
  binaries.
- One card is a genuine production evidence gap in `crates/cargo-allow`; it
  warrants a focused contract/guard follow-up, not a bulk tuning campaign.
- Twelve cards are intentionally scoped to committed fixtures and structural
  identity tests. No detector/actionability defect is established by this run;
  a future production-vs-fixture presentation issue may be filed separately if
  a consumer needs that distinction.
- No automatic changes were made to cargo-allow. The source issue
  `unsafe-review#541` should receive this receipt and the one-seam follow-up
  decision after this report is merged.

## Proof commands

The report-producing run used the issue matrix above. The swarm PR must also
pass:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- check-dogfood
cargo run --locked -p xtask -- check-pr
git diff --check
```
