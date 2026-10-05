# Standalone installation-receipt admission — 2026-10-05

This bounded [#1887](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1887)
repair follows the late [PR #2352 finding](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2352#discussion_r4181416783)
and [single-writer admission](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1887#issuecomment-5989913675).
[WORK-1887](../../plans/work-specs/examples/UNSAFE-REVIEW-WORK-1887.toml) owns its invariants and acceptance.
The [incremental roadmap](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2223#issuecomment-5984305987)
and umbrella #1887 remain open.

## Problem and behavior

At base `2f648512ad0805578b834010f358edd4a15e0e69`, matching identity fields
admitted standalone source binding without checking the supplied installation
receipt's schema or upstream rows. A failed `identity:source-sha` row, a wrong
schema, or missing required rows could acquire verified source binding.

The probe now requires `unsafe-review/installed-qualification/v1`, a nonempty
row array, uniquely named object rows with integer exit values, all 19 required
v1 assertions, and every supplied row passing. Boolean, floating-point and
string exits are rejected. Passing additive metadata and rows remain readable;
a failed additive row still prevents admission. Existing source/binary/version/
lockfile/toolchain identity checks run only after valid upstream qualification.

Invalid or failed input adds a failed `upstream-install-qualification` row,
records `upstream_qualification_status` as `invalid` or `failed`, leaves
`source_binding` unknown and returns `not_qualified` before commands.
The stdout JSON and saved receipt expose the bounded stop reason. No-provenance
use remains a capability observation. The receipt proves supplied-input
consistency, not independent build attestation or safety.

## Executed proof

Tests-first head `d7ba373be681f1b2df3b5d4e14ef4c03b71522f4` changes only the Python controls.
The exact base probe SHA-256 is
`97b0b9103a6c3ef59acf9e86d90e3cb000aafe7b6d58ac5f30ba1d565fb3a4cf`.
Local stdlib-Python RED ran 16 test methods: invalid-input controls exposed
22 assertion failures and four missing-diagnostic errors, while complete
passing admission, additive compatibility, identity mismatch, no-provenance
and the three writer controls retained their expected behavior.

After the repair, the same 16 methods pass. The controls execute the actual
embedded receipt writer and real probe entry point, with subprocesses mocked
at the first command boundary. They assert rejection before any command, unknown
source binding, non-qualification and explicit invalid/failed diagnostics.
Positive cases establish admission and identity checks; they deliberately stop
at that boundary and do not claim the full consumer fixture ran.

The unchanged test-file SHA-256 is
`b42468038dfacdcf04defa3943db5ec09092fe04d6a6981b63637dffd7ac693b`;
repaired probe SHA-256 is
`5ff3d6095b3ee3f4e9d25cef0b87092fbb31a05ea96207e9b33e6a22c4d0f106`.
Current exact-head review, required checks and protected merge are recorded
in the follow-up PR and canonical #1887 closeout.

## Preserved proof and boundaries

[PR #2352](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2352)
and its [hosted GREEN](https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/37271592490/job/111639570647)
remain valid historical proof: Source candidate
`735fb7ba1107b79a9c18bd9b6bf169eeeae68cd1`, all 19 upstream rows passing,
20 actual consumer assertions, seven verifier controls and three receipt tests.
The normal workflow already stops after a failed upstream writer. Its ordering,
runner, permissions and triggers are unchanged.

This source-contract repair does not rerun or repin that historical hosted receipt.
The added admission row makes a future complete probe population larger than the
historical 20; the local tests qualify this admission boundary only.
Actual Source candidate code, installed 0.3.8 control, provider/editor integration,
Windows execution, package installation, witness/site execution, calibration,
and release readiness are not expanded by these tests.

UB Review orchestration remains separate: the #2352 ready-triggered
[job](https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/37274728587/job/111649153016)
installed unsafe-review 0.3.4 and logged a failed grouped-review post despite
a successful check. This is existing provider/orchestration-owner work, not
successful candidate review or cargo-allow permission evidence.

No local Cargo/compiler, unsafe allowance, security-gate policy, credential,
runner/protection, publication or deployment changes are part of this slice.
Task materialization remains source/lightweight within the admitted bounded
budget. Rollback is a normal revert of the admission guard, controls and docs;
preserve the earlier proof and unique evidence.
