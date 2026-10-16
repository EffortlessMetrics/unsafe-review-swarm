# Explicit native PR admission

The draft caller prepares public central Rust workflow
`8ff77e5876b4a7808a58e053aa6248b1020e02d0` (workflow blob
`6107a7dbcb94ce6a5b44b2dbf063c3500eae9129`) with `profile: standard`,
`.ci/run.sh`, full history and these closed selectors:

```yaml
proof_admission: native-review-v1
whole_run_deadline: native-run-1200s
upload_artifacts: true
artifact_lane: failure-diagnostics
```

No secrets or additional permissions are introduced. The proof caller retains
only the four previously approved read scopes; cheap metadata jobs retain
contents-only access. This source selection does not prove or activate the live
allowance for the new pin.

## Actual admission and outstanding readiness

The control driver now executes its real native request/review checks instead
of the obsolete unconditional missing-central-hook refusal. It has a 120-second
total metadata deadline and a bounded event size. Only a fresh explicit
coordinator body edit on a non-draft same-repository PR can emit `admitted=true`.
Draft, Ready, synchronize, push, schedule, unrelated edits, stale subjects,
missing/dismissed/adverse exact review and API failures grant no product authority.
No request or machine approval receipt was issued by this source preparation.

The control driver also checks that all three central review-owner paths exist
as bounded file blobs at the exact protected-base SHA:

- `scripts/ci/check-pr-semantic-review-currentness.py`
- `scripts/ci/check-pr-review-convergence`
- `scripts/ci/check-pr-review-convergence-core`

All three owners landed through bootstrap #2396 in protected main
`1f99630ade468378c54057ed09635e4183ecaae8` (tree
`33f7e289f4a0e0e1a96ab80bb71f1e0fe7d033f4`). Its native product run
[38970363541](https://github.com/EffortlessMetrics/unsafe-review-swarm/actions/runs/38970363541)
and final review establish the bootstrap's landing, not this migration's proof.
The migration incorporates that base and preserves the accepted owner bytes and
modes. Readiness still checks the actual protected base on every request;
candidate-supplied tools cannot replace protected-base authority.

This source replaces the temporary legacy guard only when the migration lands
through its own required checks and review gates. The two metadata controls
retain the bootstrap repair: existing contents-only token authentication,
missing-token refusal, and isolated Python imports. The token is step-scoped
and is not forwarded to the proof script. The obsolete legacy-router test suite
is replaced by `.ci/test-admission.py`, including authenticated driver, import
isolation, draft/event refusal, partial-rerun/result-binding and review-drift
controls. The three owner suites remain unchanged.

After allocation the immutable central `native-review-v1` hook independently
rechecks authenticated coordinator/admin authority, current protected base,
head/native merge/tree/parents, native request/review/thread state and run/attempt.
It executes the three owner programs from protected-base Git objects in an
isolated environment, then repeats native authority before proof entry.
Credentials remain isolated from Cargo. The consumer metadata precheck is not a
substitute for that central check and cannot establish reviewer independence.

Before proposing one qualification, verify live allowance for this exact pin,
recheck the landed protected-base owners, confirm immutable
runner tools/runtime eligibility, and obtain exact-candidate independent review.
No dispatch is authorized by this source checkpoint. Actual runtime/artifacts,
required-result liveness/binding, cleanup and resource fit remain unproven.

## Subject and coordinator

The sole coordinator remains `EffortlessSteven`; this is source policy, not an
access grant. Native event sender, original/triggering actor and request actor
must agree. Head, current main base, native integration SHA/tree and ordered
parents must match current public metadata. Head/base/body/draft or review drift
revokes the request. Native `Unsafe Review Rust Result` requires successful
admission, successful central proof and a matching native receipt; absent or
skipped proof cannot be green. Protection settings remain unchanged.

## Receipt and request protocol

After independent review of the actual head/base/native integration tree, the
coordinator can preserve its disposition in a native GitHub PR review anchored
with `commit_id` equal to the reviewed head. The single-line machine record is:

```text
<!-- unsafe-review-independent-review-v1: {"repository":"EffortlessMetrics/unsafe-review-swarm","pr":PR_NUMBER,"head":"HEAD_SHA","base":"BASE_SHA","integration":"NATIVE_MERGE_SHA","tree":"MERGE_TREE_SHA","verdict":"approved-for-proof","review_kind":"independent"} -->
```

All SHA placeholders must be exact 40-character lowercase values. Native
review ID, commit identity, author, submission state and timestamp are read
from GitHub, not trusted from the body. `COMMENTED` receipts support independent
agent review in the one-contributor model; they do not impersonate a GitHub
maintainer approval. The coordinator attests the independently produced review;
the metadata gate cannot prove reviewer independence itself. A newer receipt
for this exact subject supersedes the earlier one; blocked or dismissed receipts
cannot authorize proof. Arbitrary advisory bot findings remain separate.

The coordinator then explicitly replaces the PR body's request record:

```text
<!-- unsafe-review-admission-v1: {"repository":"EffortlessMetrics/unsafe-review-swarm","pr":PR_NUMBER,"head":"HEAD_SHA","base":"BASE_SHA","integration":"NATIVE_MERGE_SHA","tree":"MERGE_TREE_SHA","coordinator":"EffortlessSteven","request_id":"NEW_32_HEX_NONCE","review_id":NATIVE_REVIEW_ID,"issued_at":"UTC_YYYY-MM-DDTHH:MM:SSZ"} -->
```

Use a distinct request ID for ordinary re-requests. The ID detects changes
relative to the immediately previous body; it is not a global single-use grant.
A deliberately restored record is still subject to the authenticated actor,
short freshness window and current exact review. No persistent marker alone
provides authority. The review must precede the request; the body edit must
follow within 60 seconds, and admission must begin within ten minutes. The
previous event body must not contain the same nonce. Unknown/duplicate fields,
ambiguous records and missing body changes are refused. Leaving a request in
the body does not authorize later Ready/synchronize or unrelated edits. A new
explicit request remains subject to current exact-source review and all checks;
a label or Boolean is never sufficient.

## Deadline and interruption behavior

Original native `created_at` plus 1,200 seconds is retained across stages;
`run_started_at`, request time and product-entry time never reset the origin.
The consumer binds the fixed public native run/attempt endpoint to the runtime
repository/workflow/PR head/base/merge subject and refuses retries, expiry,
missing metadata and identity mismatches. All three metadata phases are bounded
by the process supervisor to five seconds plus one second termination grace.
The response is capped at 2,000,000 bytes.

Proof uses remaining time minus 180 seconds. Finalization is capped at 110
seconds; its 56-second reserve accounts for ten seconds termination grace, six
seconds for final metadata and forty for external phases. Boundary tests also
charge the post-proof metadata query and proof termination grace. Original
product failures survive metadata/finalizer failures. Owned process groups are
killed even after leader exit and pipe draining is bounded; see
[interruption supervision](INTERRUPTION_SUPERVISION.md).

The new central selector supervises executing setup/proof/diagnostic bodies and
checks original deadline around external actions. It does not establish
termination of unallocated queued runs, external action post-steps, escaped
sessions, host cleanup or GitHub terminal-status latency. Neither source guards
nor reserves prove cold native product fit. Warm local 17m06 excluding setup,
cold build and cleanup is not twenty-minute qualification.

## Bounded failure diagnostics

The selected central lane validates and publishes only exact private staged
`summary.md`, `metadata.json` and optional validated `test-diagnostics.json`.
Retention is seven days and publication failure is non-fatal to the original
product result. Raw logs, unrelated workspace files and symlinks are excluded.
This replaces the old missing-upload-contract source blocker; native upload,
retention and cleanup still need actual qualification.

The pinned actual publisher accepts a real consumer-produced failure fixture
and rejects tampered source identity. The compatibility test checks the exact
central Git blob before executing its publisher/owner functions, with no API,
credentials or product build:

```bash
python3 .ci/test-central-contract.py /path/to/exact/pinned/rust.yml
```

The native admission suite executes the shipped workflow program, including its
actual main driver. Positive fixtures produce outputs only after all checks;
missing protected-base owners produce no output. Other focused controls cover
metadata trickle/stall/cancellation/size limits, descendant interruption,
original failure propagation and deadline arithmetic. They are local source
evidence, not native execution receipts.

## Ownership and integration

#2380 retains `ci.yml`, `.ci/**` and `xtask/src/ci_routing_contract.rs`.
#2394 owns its separate Windows baseline aggregate; it is unmodified and not
consumed here. Any combined candidate requires explicit integration and fresh
seam review before qualification. #2395's interruption source was consumed
unchanged; historical evidence is retained in the PR conversation and
[migration handoff](../handoffs/2026-10-14-contabo-consumer.md).
