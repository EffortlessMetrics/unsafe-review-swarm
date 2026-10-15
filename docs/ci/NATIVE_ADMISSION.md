# Explicit native PR admission

The consumer proof needs a native PR check when a merge queue is unavailable.
A dispatch or a merge-group check alone does not establish that PR result.
[GitHub's PR event contract](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#pull_request)
uses the native PR merge commit as `GITHUB_SHA`.

The CI workflow therefore observes ordinary PR lifecycle events cheaply. The proposed protocol admits only
an **explicitly edited coordinator body request**, with current exact review.
The executable driver presently refuses every admission because the active
central pin cannot securely revalidate authority after queue wait and before
Cargo. The protocol is locally prepared, not activated. Open, Ready, synchronize, draft transitions, close, title
edits and unrelated body edits provide no product authority. Lifecycle changes
cancel obsolete proof; an unrelated body edit does not silently cancel a valid
proof. The metadata-only `Unsafe Review Rust Result` fails when admission or
proof is absent, so a skipped call cannot appear as a successful product verdict.
The actual native check name/binding still requires qualification; no protection
or required-check setting changes are included.

## Subject and coordinator

The sole coordinator is `EffortlessSteven`, whose current repository admin
permission was verified during this source repair. This is an explicit source
policy for the existing one-contributor operating model, not a new repository
access grant. The authenticated event sender, original/triggering Actions actor
and request coordinator must all match. Bot or fork source, draft/closed PRs,
unexpected bases, unrelated events and rerun attempts are refused before the
central call. Changing the coordinator requires a reviewed source change.

Admission requires the current head, current main base, native integration SHA
and integration tree. The gate checks both the event and current public PR
metadata, the native integration commit's tree and ordered base/head parents,
and refreshes PR metadata after reading reviews. New head/base/integration/body
or draft state invalidates the request. Public API errors, rate limits, unknown
mergeability and bounded-pagination exhaustion refuse admission; there is no
permissive fallback and no credential or permission expansion.

## Receipt and request protocol

This protocol is source preparation only. **Do not issue an admission request
while the central failure-diagnostic contract remains unresolved.** No request,
label or dispatch was activated for this draft.

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

The prepared validator refreshes reviews at the final admission check; the
prepared final result also refreshes current review/request authority after
proof. These checks do not close the queue-wait race. An approval can be
withdrawn after control admission while the central proof waits for a runner.
The immutable central workflow must perform authenticated current-authority
revalidation after allocation and immediately before candidate compilation,
without handing that authority to candidate code. Missing/dismissed/adverse
approval, changed head/base/native merge/tree or revoked request must refuse
entry and propagate failure. The current native driver emits no admitted output
until that separately reviewed, published and activated contract exists.

The final metadata result requires the entire central call to succeed and its
successful result/receipt to match the native repository, SHA, run, attempt and
standard profile. It does not re-label a dispatch, cached proof or skipped job
as a native PR success. Source updates cancel old runs and require fresh
admission. This preparation does not prove runtime qualification or close every
possible race after metadata admission; actual event/source/result bindings
must be observed before cutover.

## Focused verification

`python3 .ci/test-admission.py` executes the shipped inline workflow program,
with positive exact-subject input and negative controls for events, actors,
trust, drafts, persistence, timestamps, missing/dismissed/adverse reviews,
head/base/integration/tree/parent drift, racing metadata, and wrong/skipped native
results. The Rust routing-contract tests also execute this suite. Tests use
fixtures and create no live approval or product execution.

The public review endpoint supports anonymous public-resource reads, as
[documented by GitHub](https://docs.github.com/en/rest/pulls/reviews#list-reviews-for-a-pull-request).
No native Actions or pool qualification was executed. The active central pin and
its honest failure-artifact readiness refusal remain unchanged and parent-owned.


The governed reusable proof caller now declares exactly `contents: read`,
`actions: read`, `checks: read` and `pull-requests: read`. The three additional
read scopes were explicitly approved for this caller; the metadata jobs retain
contents-only access. There are no write grants or inherited secrets. This
matches the active central repository-policy job's declared read requirements,
even while that optional job is skipped. The earlier automatic draft CI run on
`3cc4e5f5` reported startup failure and zero jobs; post-change native cheap
metadata is used to diagnose startup, not to qualify product execution.
The pre-Cargo admission and diagnostic-retention blockers remain unchanged.


The parent confirmed the same pre-proof gap in active `a3125de` and public
`15f2c4d`/internal `ab217deb`. The minimum central prerequisite is a central-owned
read-only recheck immediately before every proof-entry path, including recovery:
native semantic-currentness and convergence owners from trusted protected-base
source; current request/actor/head/base/integration validation; credential
isolation from Cargo; and refusal failing the aggregate without fallback
normalization. Consumer body-record fixtures cannot substitute for those owner
contracts or create authority. Native `auto_merge_enabled` is a supported future
request event, but the existing authenticated requesting actor must generate an
observable event; a GITHUB_TOKEN transition is not assumed observable. No such
transition was attempted here. The parent owns that dependency and activation.

## Twenty-minute qualification and ownership

The late qualification requested under [#2393](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2393)
requires an explicit non-draft, merge-ready, independently reviewed current
native subject and at most twenty minutes for the whole run. This includes
setup, routing/control, cold compilation, product proof, diagnostic validation,
finalization and cleanup. The previous 65-minute script limit did not implement that contract; the
remaining pinned 75-minute central job limit still does not implement it. A timeout-only edit
cannot establish workload fit. The warm local 17m06 span excludes setup, cold
build and cleanup and is not native twenty-minute qualification.

Before activation, the CI owner must reconcile a trusted native run start and
absolute deadline with the immutable central caller, including queue/recovery
paths and bounded cancellation/cleanup. Candidate-provided timestamps or a
deadline reset at product entry cannot establish a whole-run bound. Central
publication alone does not establish protected-base semantic-currentness and
convergence owners in this consumer, reviewed pin/adapter adoption, or runtime
prerequisites. The current admission driver remains unconditionally refused;
no product dispatch or live pin change is part of this source repair.

[#2380](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2380)
retains sole ownership of `ci.yml`, `.ci/**` and
`xtask/src/ci_routing_contract.rs`.
[#2394](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2394)
owns the separate Windows baseline aggregate and does not edit those surfaces.
Any combined integration must consume both reviewed sources explicitly and
receive fresh review of their seams before proposing qualification. Preserve
the native `Unsafe Review Rust Result` producer and existing protections.

The result-publication repair makes failure to publish the core result receipt
fail closed (78 after successful commands, otherwise the original failure).
`python3 .ci/test-run.py` exercises that failure, real GNU timeout with only
the fixture duration shortened, and a SIGTERM-terminated producer. Each must
prevent success and later producer execution as applicable. These controls do
not qualify native Actions cancellation, cleanup or the whole-run deadline.


### Native deadline adapter source checkpoint

The pure helper from `d7e5f5ffffc0456520e8b60b0650a5ac4d5924ad`
(base `b5355e2f917cad5132f5e1b72e2d8d43510b7d8a`, tree
`b432f2cc08ae41dff00947ab33a194d4f714881c`) was fetched from
`fix/ci-whole-run-deadline`, verified as exactly the two owner-scoped files,
reviewed and cherry-picked unchanged. It is consumed by
`.ci/native_deadline.py`; #2394 source remains separate and unmodified.

Before Cargo, the adapter fetches the fixed public GitHub API native
run/attempt endpoint over TLS without credentials or a candidate URL override.
It requires this repository, CI workflow path, in-progress PR run, first attempt,
and matching native run ID and PR head/base subject against the runtime event.
It uses native `created_at`, never `run_started_at`, request-body time, a supplied
environment timestamp or product-entry time. Missing/unavailable/oversized
metadata or mismatches refuse with 78. Actual Actions payload shapes and public
API availability are not qualified by fixtures; absence refuses rather than
falling back to a fresh local origin.

The script subtracts elapsed setup/control/queue time from the 1200-second
whole-run origin and reserves 180 seconds before proof. GNU timeout bounds the
real proof process group and allows 30 seconds of termination grace within that
reserve. Before finalization it fetches the same native origin again, caps
finalization at 110 seconds plus ten seconds termination grace. Its helper
reserves 50 seconds, accounting for that ten-second grace while preserving
40 seconds for external phases even when proof uses its full termination grace. A final fresh deadline check refuses success
if the origin has expired. Existing product failures remain primary when
metadata or finalization fails. No candidate metadata outputs can admit proof.

This is bounded consumer-entrypoint enforcement, not a whole-run watchdog. It
cannot stop central checkout/toolchain installation before entry, enforce
central uploads/cleanup after return, or terminate a stalled metadata read
within a total wall deadline. Runtime clock and native API behavior are still
qualification assumptions. Reserve accounting does not prove those external
phases fit their reserves. The immutable central contract must own an absolute
whole-run cancellation/cleanup bound and authenticated review authority before
admission can activate. Native result/protection requirements are unchanged;
the actual driver still refuses and no pin or permissions changed.

Focused controls: `python3 scripts/ci/test_qualification_deadline.py`,
`python3 .ci/test-native-deadline.py`, `python3 .ci/test-run.py` and
`python3 .ci/test-admission.py`. They distinguish origin reset, native identity
mismatch, setup already spent, expiry/reserves, entry refusal, post-proof
metadata failure and original product failure propagation. Fixtures make no
native runtime fit, cancellation or cleanup claim.
