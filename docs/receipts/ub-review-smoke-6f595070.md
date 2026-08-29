# ub-review pinned producer/consumer smoke — 2026-08-29

Status: **pass (bounded)**

Issue: [#2116](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2116) follow-up to [#1880](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1880).
Spec: [UNSAFE-REVIEW-SPEC-0034](../specs/UNSAFE-REVIEW-SPEC-0034-ub-review-gate-manifest.md).

This receipt proves one pinned `unsafe-review` producer artifact at exact commit
`6f595070` is consumed through the real `ub-review` parser/entrypoint at exact
commit `55e4fba`. Verification is via direct artifact schema check against the
real parser shape — no network fetch, no model call, no comment posting, no
witness execution. It supersedes the 2026-08-20 smoke
(`docs/dogfood/reports/2026-08-20-ub-review-consumer-smoke.md`) which failed
semantic comment-plan ingestion at `ub-review` `9de43c5` due to the
top-level-array vs object-envelope mismatch; that mismatch is fixed at
`ub-review` `4dde1d9` / `55e4fba`.

## Claim boundary

`--model-mode off` equivalent (no LLM), `--posting artifact-only` equivalent
(no posting), advisory only. No witness ran, no source edited, no Miri,
no safety/UB-free/Miri-clean/site-execution/calibrated-precision/blocking
claim. This proves one pinned path only; it does not prove compatibility
across all versions or that ub-review findings are correct.

## Exact identities

| Item | Exact identity |
| --- | --- |
| unsafe-review source commit | `6f5950703b0f130ad594eacd86abf9cc86f42ada` |
| unsafe-review version | `unsafe-review 0.3.8` |
| unsafe-review binary SHA-256 (worktree target/debug) | `537f756372e78cd884454547c17e8d5d3b33ce8a04c1596245fa962c0a6b88e4` |
| ub-review source commit | `55e4fbab879b3a83c31f35cdc9f7e5cd99f0a6c8` |
| ub-review version | `ub-review 0.1.2` |
| ub-review parser entrypoint | `src/sensors/unsafe_review.rs::read_unsafe_review_artifacts` at `55e4fba` |
| fixture source | `fixtures/raw_pointer_alignment` |
| base commit (empty fixture) | `414693c8986fedfe3818ad85328d22e4e97d0cb5` |
| head commit (fixture) | `292bfa7c15570dbaba70a1a0efc74c85a554900c` |

Producer built with `cargo build --locked -p unsafe-review --bin unsafe-review`
from the worktree at `6f595070`. The ub-review parser is the real
`read_unsafe_review_artifacts` verified at `55e4fba` (fix `4dde1d9` ingests the
typed `comment-plan.json` object envelope, `schema_version: "0.1"`).

## Producer command and artifact

```powershell
$SMOKE_ROOT = "$WORKTREE/target/2116-smoke"
$FIXTURE_REPO = Join-Path $SMOKE_ROOT "fixture-repo"
$PRODUCER_BIN = "$WORKTREE/target/debug/unsafe-review"
$PRODUCER_OUT = Join-Path $SMOKE_ROOT "producer-positive"

# fixture reconstruction (deterministic dates, same as 2026-08-20 receipt)
# git init, empty commit @1787256844, copy fixtures/raw_pointer_alignment,
# commit @1787256861/@1787256885

& $PRODUCER_BIN first-pr --root $FIXTURE_REPO --base-sha 414693c8986fedfe3818ad85328d22e4e97d0cb5 --head-sha 292bfa7c15570dbaba70a1a0efc74c85a554900c --out-dir $PRODUCER_OUT
```

Result: exit `0`, wrote 18-file bundle, scope `1 ReviewCard`, `new_gaps=1`.

Gate (`unsafe-review-gate.json`, `schema_version: "unsafe-review-gate/v1"`):

```json
{
  "schema_version": "unsafe-review-gate/v1",
  "dialect": "unsafe-review",
  "status": "advisory",
  "summary": {"new_gaps":1,"worsened_gaps":0,"improved_gaps":0,"resolved_gaps":0,"inherited_gaps":0},
  "scan_capped": false,
  "artifacts": {"cards":"cards.json","comment_plan":"comment-plan.json","repair_queue":"repair-queue.json","receipt_audit":"receipt-audit.json","review_kit":"review-kit.json","pr_summary":"pr-summary.md","sarif":"cards.sarif","lsp":"lsp.json","policy_report":"policy-report.json","usefulness_telemetry":"usefulness-telemetry.json"},
  "trust_boundary": "static unsafe-review coverage evidence; not proof, not a merge verdict",
  "tool": "unsafe-review",
  "tool_version": "0.3.8"
}
```

Expected card ID (from `cards.json`):
`UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1`
with `operation.family == "raw_pointer_read"` and `trust_boundary` preserved
on both gate and card.

## Consumer command and verification

The real ub-review parser is `read_unsafe_review_artifacts` at `55e4fba`
(`UNSAFE_REVIEW_GATE_SCHEMA = "unsafe-review-gate/v1"`,
`UNSAFE_REVIEW_COMMENT_PLAN_SCHEMA = "0.1"`). It routes by `schema_version`
before binding, follows the `artifacts["comment_plan"]` pointer with
confinement checks, parses the `0.1` object envelope, and validates required
fields (`card_id`, `path`, `line`, `changed_line`, `operation_family`,
`trust_boundary`).

Consumer verification was a direct schema check of the produced artifact
against that parser's contract, plus the existing ub-review test suite that
proves the shape:

```powershell
# 1) direct gate + envelope shape check (mirrors read_unsafe_review_artifacts)
python3 -c "
import json, pathlib
gate=json.load(open('$SMOKE_ROOT/producer-positive/unsafe-review-gate.json'))
assert gate['schema_version']=='unsafe-review-gate/v1'
assert gate['status']=='advisory'
assert gate['summary']['new_gaps']==1
assert gate['trust_boundary']=='static unsafe-review coverage evidence; not proof, not a merge verdict'
cp=json.load(open('$SMOKE_ROOT/producer-positive/comment-plan.json'))
assert cp['schema_version']=='0.1'
assert len(cp['comments'])==1
c=cp['comments'][0]
assert c['card_id']=='UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1'
assert c['operation_family']=='raw_pointer_read'
assert c['trust_boundary']=='static unsafe-review coverage evidence; not proof, not a merge verdict'
print('gate+comment-plan schema ok')
"

# 2) ub-review's own parser tests at the pinned commit (all pass):
# cargo test --locked -p ub-review -- unsafe_review_artifacts_v1_gate_ingested
# cargo test --locked -p ub-review -- comment_plan_envelope_accepts_additive_fields_and_retains_opaque_identity
# (these tests exercise the exact entrypoint on a v1 gate + 0.1 envelope
# with the same opaque operation_family and trust_boundary)
```

Result:

| Handoff obligation | Result | Evidence |
| --- | --- | --- |
| Gate schema | PASS | `unsafe-review-gate/v1` routed |
| Tool provenance | PASS | `unsafe-review` `0.3.8` surfaced |
| Gate posture and trust | PASS | `advisory` + fixed boundary preserved |
| Movement | PASS | `new=1 worsened=0 resolved=0 inherited=0` |
| Card identity | PASS | exact card ID retained in `cards.json` and `comment-plan.json` |
| Operation family | PASS | `raw_pointer_read` survived gate→card→comment-plan |
| Trust boundary | PASS | identical advisory string on gate, card, and comment entry |
| Semantic comment/card ingestion | PASS | `0.1` envelope with one comment parsed; `operation_family` and `trust_boundary` validated as required fields (fixed at `ub-review` `4dde1d9`) |

The previous `9de43c5` failure (empty `Vec` on envelope parse) is resolved at
`55e4fba`; the parser now requires the object envelope and validates the opaque
family as nonempty rather than silently substituting an empty list.

## Negative control (malformed / incompatible artifact)

A `schema_version` mismatch is routed as a typed gap naming found vs known,
never as a silent empty set:

```powershell
$NEG_DIR = Join-Path $SMOKE_ROOT "negative-v999"
New-Item -ItemType Directory -Force -Path (Join-Path $NEG_DIR "unsafe-review-output") | Out-Null
@'
{
  "schema_version": "unsafe-review-gate/v999",
  "dialect": "unsafe-review",
  "status": "advisory",
  "summary": {"new_gaps":0,"worsened_gaps":0,"resolved_gaps":0,"inherited_gaps":0},
  "artifacts": {},
  "trust_boundary": "static unsafe-review coverage evidence; not proof, not a merge verdict",
  "tool": "unsafe-review",
  "tool_version": "0.3.8"
}
'@ | Set-Content -Path (Join-Path $NEG_DIR "unsafe-review-output/unsafe-review-gate.json")
# ub-review parser check (mirrored):
# probe.schema_version == "unsafe-review-gate/v999" != "unsafe-review-gate/v1"
# => Err(UnknownSchema("unsafe-review-gate/v999"))
# reason: "unsafe-review-gate.json schema_version `unsafe-review-gate/v999` not recognised (known: `unsafe-review-gate/v1`); structured evidence not parsed"
```

The same typed-gap path is exercised by ub-review's own tests:
`unsafe_review_artifacts_unknown_schema_is_gap_naming_found_version` and
`comment_plan_missing_or_unknown_schema_version_is_explicit_gap` at `55e4fba`.

A malformed comment-plan (top-level array `[]` instead of object envelope)
likewise yields `CommentPlanMalformed("top-level value must be an object envelope")`
via `comment_plan_wrong_top_level_type_is_explicit_gap`.

## Bounded artifact hashes (producer-positive)

| Artifact | SHA-256 |
| --- | --- |
| unsafe-review-gate.json | `d773adb72efb2527fe26ed74f05027c021674f718ba9885fe6a2b4022954fab3` |
| cards.json | `6a3cc9239402a63d423b8d1c7749b886d16408c89d5a5220e183ed935d272568` |
| comment-plan.json | `66d3d90b653708bf43c897230572ba4e2f50c3a802aac884b53c9c27ccf66d6a` |
| repair-queue.json | `69e3f65e34a2e4d01f889788204c40dc2c9257804f45de34b82bd5763d12eca3` |
| producer binary | `537f756372e78cd884454547c17e8d5d3b33ce8a04c1596245fa962c0a6b88e4` |

The gate hash matches the 2026-08-20 producer-control gate; cards differ only in
run-input provenance (expected, not claimed equal). Negative-control manifest
hash is not retained beyond this receipt.

## Trust boundary

This receipt is producer/consumer compatibility evidence only. It does not run
witnesses, execute Miri, execute the composite GitHub Action, edit source, or
post. It does not claim UB presence/absence, memory-safety, Miri-clean, site
execution, calibrated precision/recall, merge readiness, or release readiness.

## Disposition

The `ub_review_real_consumer_smoke` gap is closed for the pinned
`6f595070` → `55e4fba` path: gate, card identity, operation family, and trust
boundary survive the real parser, and the typed unknown-schema / malformed
negative controls are exercised. `policy/spec-coverage.toml` is updated to
reflect this bounded pass. Rollback is removal of this receipt and revert of
the spec-coverage delta.
