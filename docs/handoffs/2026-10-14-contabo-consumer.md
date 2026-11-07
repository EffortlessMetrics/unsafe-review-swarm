# Contabo consumer migration draft — 2026-10-14

Tracking: [em-ci #37](https://github.com/EffortlessMetrics/em-ci/issues/37).
This branch owns only the swarm CI caller, repository proof extraction,
focused contract tests, toolchain pin and corresponding policy/docs.
No open migration PR overlapped this writer at admission. The source baseline
is `b5355e2f917cad5132f5e1b72e2d8d43510b7d8a`.

## Verified source contract

The current central activation source names
`a3125de962a74e7ec2e127d41d7532f66c69334d`. Its Rust workflow blob is
`6d202cfd6861bb359381dfb128c1574c04505285`; it selects standard capability,
has a 75-minute standard job, accepts exact Rust 1.98.1 or 1.95.0,
and exports `CARGO_TARGET_DIR=$RUNNER_TEMP/cargo-target`. This repository's
manifest requires Rust 1.98, so its toolchain is pinned to 1.98.1. No manifest
MSRV reduction or dependency change is included.

The caller removes physical routing, inventory polling, runner-read secret,
host cleanup and consumer cache ownership. Its real proof retains fmt,
clippy, structured `xtask ci-test` plus doctests, rustdoc and `check-pr`.
The existing non-empty all-Markdown skip remains the only test skip.
Synchronous command status, a 65-minute proof timeout and run/attempt-keyed
atomic receipts prevent stale success from satisfying a later attempt.
Finalization gets four minutes and cannot replace the product exit status.
Advisory ub-review is unchanged and separate.

## Focused local evidence

Reproduction:

```bash
bash -n .ci/run.sh .ci/core.sh .ci/finalize.sh
python3 .ci/test-run.py
python3 .ci/test-admission.py
cargo fmt --all --check
cargo test --locked -p xtask ci_routing_contract
cargo run --locked -p xtask -- check-policy
```

Bash syntax, six orchestration tests, 16 admission tests, 36 scoped Rust tests, formatting, scoped
`cargo clippy --locked -p xtask --all-targets -- -D warnings` and `git diff
--check` passed locally. Full product proof and native runner qualification
were not run. The existing policy aggregate is not claimed green.

The Python tests execute the shipped Bash scripts with controlled Cargo
responses: exact command/argument preservation, Markdown-only skip, each
product step failing with short-circuit propagation, stale success replacement,
JSON metadata escaping, exclusion of raw output, diagnostic rejection without
result masking, source/runtime mismatch refusal, and failed staging allocation without artifact-output exposure. They do not execute the
product assertions or qualify the pool.

The Rust tests retain the advisory identity controls, diff selector and private
failure-staging controls and exercise the extracted script contract.
`source-divergence` reports `new_source_commits=0` on the baseline; the source
head is the already acknowledged `735fb7ba1107b79a9c18bd9b6bf169eeeae68cd1`.
Local Rust uses 1.98.1, with Cargo/Rustup/build storage under `/tmp`.

## Qualification blockers and next owner

1. **Failure artifact publication is missing from the activated central
   contract.** Its proof interface accepts `script`, `profile`, `fetch_depth`,
   `force_hosted` and optional `result_script`; it exposes no consumer artifact
   upload hook. The consumer retains bounded summary/metadata/optional validated
   diagnostics and job summaries, but emitted file paths are not an upload.
   the internal `check_ci_routing_contract` (via `xtask check-policy`) therefore deliberately fails with this
   named existing obligation, and `check-pr` cannot pass until it is resolved.
   The central owner must supply a reviewed exact-file, seven-day, non-fatal
   publisher before this consumer can qualify. Do not bypass this by direct
   runner selection, inherited secrets or shell runtime-token extraction.
2. Exact reviewed consumer source still needs explicit pre-merge admission,
   native run/attempt/job and runner evidence, real product assertions,
   central result receipt, artifact behavior and cleanup. Neither local
   contract tests nor a historical platform canary prove those properties.
3. Preserve current protection until the new nested native result binding,
   required-check liveness and rollback are proven. This PR changes no
   protection, required checks, permissions, runner policy or credentials.

The parent reports em-ci #228 merged at
`ab217deb99fb673103975a4fbea6b66f62d56a38` and the two missing live repository
grants applied with the selected workflow set unchanged. This consumer does
not independently audit those live grants. Full canonical configuration audit
remains NOT_PROVEN because organization-variable/package reads are unavailable.

No workflow dispatch, expensive native execution, release or deployment was
performed. Product execution is absent from Draft/Ready/synchronize and ordinary
push; only a fresh coordinator body-edit request with exact independent
review and native integration binding can enter this lane. Other workflows and their existing obligations are unchanged.

## Rollback and claim boundary

Before merge, close or repair this draft; no live consumer/protection cutover
has happened. After a future authorized source merge, revert the migration
commit together with its policy and contract-test changes to restore the prior
proof producer. Do not restore or change protected bindings without a separately
reviewed transaction. Do not treat source rollback as permission to dispatch.

This evidence establishes extraction and orchestration behavior only. It does
not establish a complete migration, pool fit, product qualification, retained
failure artifacts, native required-status success, full configuration audit,
publication readiness or unsafe-correctness claims. Keep the draft open with
the central artifact seam as the next bounded owner action.


## Admission repair continuation

Executor and custody were recovered on the existing clean writer branch at
`07da55115fe93c41e54ac3662b556a09a034838a`; no competing writer was created.
The merge-group/Boolean-dispatch-only proposal did not supply a native PR check
without a queue and did not independently bind reviewed source. It is replaced
by [explicit native PR admission](../ci/NATIVE_ADMISSION.md). A changed body
request, authorized coordinator, native review identity and current
head/base/merge/tree are all required. Drafts, routine lifecycle events,
stale/persistent requests and missing approval allocate no product proof.
A cheap aggregate refuses absent/skipped proof and requires the exact central
native result receipt. Workflow permissions remain contents: read.

The coordinator's current admin permission was verified read-only. Local
anonymous public-API probing was unavailable in this executor (URLError), so
network/API behavior on native Actions remains unqualified. Fixtures exercise
the actual shipped program; they are not live admission or required-check proof.
No central source or pin changed. The diagnostic publisher is a separate
parent-owned preparation/activation dependency; its readiness refusal remains.


### Queue-wait authority boundary

Independent challenge reproduced review revocation during control admission;
review authority is now refreshed at the final control check and after proof.
The request ID is explicitly a body-transition identifier, not a globally
unused grant; intentional restoration still requires current actor/time/review.
Neither fix prevents authority becoming stale while an owned job waits in the
central queue. Active `a3125de` has no authenticated immutable pre-Cargo hook.
The native driver therefore refuses before producing `admitted=true` or
allocating product proof. There is no label/Boolean override. The parent-owned
central dependency must securely revalidate current review/request and exact
head/base/native merge/tree immediately before compilation, after queue wait.
Keep this source unqualified until that contract is reviewed and activated.

The automatic native CI run on `3cc4e5f5` failed startup with zero jobs. Static
inspection shows the active central repository-policy job requests read scopes
beyond the caller's contents-only declaration. No permission was expanded;
startup annotations were not available through permitted connector endpoints.
The compatible scope/central-contract disposition remains parent-owned.


Central minimum prerequisite (parent independently confirmed): central-owned,
read-only revalidation at every pre-proof/recovery entry after queue wait,
protected-base semantic-currentness/convergence owner contracts, current
request/actor/head/base/integration binding, credential isolated from Cargo,
and refusal failing the aggregate without fallback normalization. The consumer
source currently emits no admission authority and cannot replace this gate.
Native auto-merge request support requires an observable transition by the
existing authenticated actor; no GITHUB_TOKEN transition or label is assumed
sufficient. No live request/label/dispatch or permission expansion occurred.


### Approved caller read-scope repair

Continuing from reviewed `e68e64ed72ce8c76a70fc89817b4002aae9f482e`, the user
explicitly approved the three required read scopes for the governed caller:
actions, checks and pull-requests. Only the reusable proof job adds those read
scopes, retaining contents: read. Metadata jobs retain contents-only permissions;
no write scope, credential, pin or live settings change is included. The policy
ledger and discriminating missing/extra/write-scope tests reflect that exact
contract. Earlier permission-compatibility observations above retain their
original proof epoch; this repair addresses that source mismatch only.
The actual admission driver still refuses before outputs or product allocation,
and the central pre-Cargo/diagnostic-retention readiness refusals remain.
