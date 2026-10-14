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


The source caller retains `contents: read`; no scope expansion was performed.
The active central definition also statically requests Actions/Checks/PR read
scopes in its optional repository-policy job, even when no result script is
selected. The automatic draft CI run on `3cc4e5f5` reported startup failure and
zero jobs; exact startup annotations were unavailable through the connector.
This caller/central permission-contract mismatch is another qualification
prerequisite. Resolve it in the parent-owned central dependency or through a
separately authorized scope decision; do not broaden access in this repair.


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
