# UNSAFE-REVIEW-SPEC-0011: PR and CI output

Status: accepted
Owner: core/spec
Created: 2026-05-17
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked plan: ../../plans/0.1.0/implementation-plan.md
See also: [UNSAFE-REVIEW-SPEC-0024: CI design](UNSAFE-REVIEW-SPEC-0024-ci-design.md)

## PR gate experience contract

### Purpose

The `unsafe-review` PR gate gives maintainers a quiet, advisory unsafe-review packet for a pull request.

The gate does **not** prove memory safety. It does **not** certify the PR as UB-free. It does **not** run Miri or any witness tool by default. It does **not** post comments or block on findings by default.

The PR gate exists to answer:

```text
What unsafe-review evidence changed?
Which ReviewCards are actionable?
What artifacts can reviewers inspect?
What witness route is worth running?
Did the tool output remain well-formed and honest?
```

The repo-control goal is that a reviewer or agent can answer why the work exists, what behavior must hold, which proof command validates it, and what claim may be made after it lands.

### 1. Core rule

The PR gate has two separate layers:

```text
artifact integrity gate
unsafe-review evidence report
```

These must never be confused.

#### 1.1 Artifact integrity gate

The artifact integrity gate **may fail CI**.

It checks whether `unsafe-review` ran correctly and produced valid, honest artifacts.

It may fail when:

```text
unsafe-review cannot run
required artifact is missing
artifact JSON/SARIF is malformed
artifact schema is invalid
artifact references unknown card IDs
comment-plan is malformed
comment-plan exceeds allowed limits
saved lsp.json is malformed
witness-plan lacks route limitations
required trust-boundary wording is missing
output makes a positive safety/proof claim
```

#### 1.2 Unsafe-review evidence report

The unsafe-review evidence report is **advisory by default**.

It may report:

```text
new ReviewCards
guard_missing cards
contract_missing cards
guarded_unwitnessed cards
missing receipts
policy-report gaps
repo posture changes
```

But these findings do not fail the PR by default.

A ReviewCard finding becomes blocking only under a future explicit policy mode with exact identity, baseline/suppression ledgers, and calibrated support. That is out of scope for the default 0.2.x PR gate.

### 2. Default user experience

A default PR workflow should do this:

```bash
unsafe-review doctor

unsafe-review first-pr \
  --base origin/main \
  --out-dir target/unsafe-review

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review
```

Then it should upload:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/lsp.json
```

The existing first-run lane already identifies this bundle and verifier as the public first-run cockpit surface.

The workflow should add a concise GitHub job summary, but should not post inline comments by default.

### 3. Required artifacts

#### 3.1 `cards.json`

Canonical machine-readable ReviewCards.

Requirements:

```text
must parse as JSON
must include schema_version
must include trust boundary
must include stable ReviewCard identities
must include operation family
must include hazards
must include obligation evidence
must include missing evidence
must include witness routes when available
```

All other PR artifacts are projections from this card set.

#### 3.2 `pr-summary.md`

Reviewer-facing front panel.

Must include:

```text
card count
top actionable card, when present
compact card table
missing evidence summary
witness route summary
artifact links or paths
trust boundary
```

Must not include:

```text
safe
sound
verified
UB-free
Miri-clean
all clear
```

except inside explicit negative wording.

#### 3.3 `cards.sarif`

Code-scanning projection.

Must include:

```text
card_id
operation family
review class
hazards
location
message with missing evidence
trust boundary
```

SARIF must not create a separate classification truth. It is a projection from ReviewCards.

#### 3.4 `comment-plan.json`

Plan-only inline-comment artifact.

Must include:

```text
mode = plan_only
policy = advisory
comments[]
trust_boundary
```

Each comment candidate must include:

```text
card_id
path
line
class
priority
confidence
operation
operation_family
next_action
witness_routes
verify_commands
selection_reason
actionability
body
trust_boundary
```

`line` must be one-based and nonzero.

Each body must include the trust boundary.

#### 3.5 `witness-plan.md`

Reviewer-readable witness routing.

Must group cards by route family:

```text
Miri / cargo-careful
Sanitizers
Loom / Shuttle
Kani / Crux
Human deep review
Unsupported / manual
```

Each entry must include:

```text
card id
why this route
suggested command, when available
what this route can show
what this route cannot prove
receipt hint
```

#### 3.6 `lsp.json`

Saved editor/LLM projection.

Must be read-only.

Must include, where applicable:

```text
diagnostics
hovers
code_actions
trust_boundary
```

Diagnostics should carry ReviewCard-derived evidence:

```text
card_id
operation
hazards
required safety conditions
evidence summary
obligation evidence
missing evidence
witness routes
verify commands
trust boundary
```

Code actions must be command-only. They must not include `WorkspaceEdit`.

Allowed action intents:

```text
copy / collect agent packet
copy / collect witness command
open related test
open PR summary
refresh diagnostics
```

Forbidden action intents:

```text
edit source
apply quick fix
insert suppression
run witness tool
post comment
approve PR
block PR
```

### 4. Gate outcomes

The PR gate should report one of these states.

#### 4.1 `ok_no_changed_gaps`

Meaning:

```text
Artifacts are valid.
No changed unsafe-review gaps were found.
```

Required wording:

```text
No changed unsafe-review gaps were found.
This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.
```

Forbidden wording:

```text
All clear.
Safe.
Verified.
No UB.
```

#### 4.2 `ok_advisory_findings`

Meaning:

```text
Artifacts are valid.
One or more advisory ReviewCards were found.
```

This is the common useful case.

The gate should pass unless the repo has opted into a future policy mode.

#### 4.3 `failed_artifact_integrity`

Meaning:

```text
The unsafe-review packet is missing, malformed, inconsistent, or overclaims.
```

This may fail CI.

Examples:

```text
missing cards.json
malformed SARIF
comment-plan references unknown card_id
comment-plan has more than 3 comments
lsp.json contains WorkspaceEdit
witness-plan says “Miri-clean”
summary says “All clear”
```

#### 4.4 `failed_tool_execution`

Meaning:

```text
unsafe-review could not run or crashed.
```

This may fail CI.

#### 4.5 `failed_explicit_policy`

Future opt-in only.

Meaning:

```text
The repo configured a blocking policy and the report found new matching debt.
```

Not part of the default 0.2.x gate.

### 5. Comment-plan policy

The PR gate must not post comments by default.

It should generate only:

```text
comment-plan.json
```

Comment-plan limits:

```text
max comments: 3
changed lines only
actionable cards only
high-confidence or high-priority only
no static_unknown
no baseline_known
no suppressed
no unchanged repo-inventory cards
```

Selection should prefer:

```text
guard_missing on changed unsafe operation
contract_missing on public unsafe API
requires_loom / concurrency model route
FFI / sanitizer route when strongly tied to changed line
```

Selection should avoid:

```text
weak reach-only cards
static_unknown cards
cards without changed-line location
cards with ambiguous macro site
cards already baseline_known or suppressed
```

Each candidate must include:

```text
selection_reason
```

Automatic posting is a later trusted-workflow feature, not this gate.

Detailed PR-comment experience rules for selection quality, actionability,
dedupe, noise budget, and the future posting model are defined in
[UNSAFE-REVIEW-SPEC-0022](UNSAFE-REVIEW-SPEC-0022-pr-commenting-experience.md).

### 6. GitHub job summary contract

The PR workflow should write a job summary.

Minimum shape:

```markdown
## unsafe-review advisory summary

Artifacts are valid.

Cards:
- Total: 3
- Actionable: 2
- Suppressed: 0
- Baseline-known: 0

Top card:
- `UR-...`
- Operation: `raw_pointer_read`
- Missing: alignment evidence
- Route: Miri / cargo-careful

Open:
- `target/unsafe-review/pr-summary.md`
- `target/unsafe-review/witness-plan.md`

Trust boundary:
Static unsafe contract review only. Not memory-safety proof, not UB-free status,
not Miri-clean status, and not site-execution proof.
```

If no changed gaps:

```markdown
## unsafe-review advisory summary

Artifacts are valid.

No changed unsafe-review gaps were found.

This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.
```

### 7. Trust boundary wording

Every public PR surface must include the trust boundary.

Minimum accepted phrases:

```text
static unsafe contract review only
not memory-safety proof
not UB-free status
not a Miri result
```

When site execution is relevant, include:

```text
not proof any unsafe site executed
```

Forbidden positive claims:

```text
safe
sound
verified
proved
UB-free
Miri-clean
all clear
site reached
blocking-ready
calibrated precision
calibrated recall
```

Allowed only inside explicit negative wording:

```text
This is not proof the repo is safe.
This is not UB-free status.
This is not Miri-clean status.
```

### 8. Policy report relationship

The default first-pr gate does not require policy report artifacts.

Policy report is explicit:

```bash
unsafe-review policy report \
  --base origin/main \
  --format markdown \
  --out target/unsafe-review/policy-report.md

unsafe-review policy report \
  --base origin/main \
  --format json \
  --out target/unsafe-review/policy-report.json
```

When included, policy report remains advisory unless configured otherwise.

Policy report may classify:

```text
new_gap
baseline_known
suppressed
resolved_baseline
expired_suppression
unmatched_baseline
invalid_ledger_entry
```

Policy report must preserve ledger evidence and explain classifications.

Policy report must not silently convert findings into blocking decisions.

### 9. Exit behavior

Default `unsafe-review first-pr` behavior:

```text
0 = ran and wrote advisory bundle
nonzero = tool/config/artifact failure
```

Default artifact checker behavior:

```text
0 = bundle valid
nonzero = missing/malformed/inconsistent/overclaiming bundle
```

Default PR gate behavior:

```text
pass if unsafe-review ran and artifacts verify
fail if unsafe-review failed or artifacts fail verification
do not fail because cards exist
```

Future opt-in policy modes may add blocking behavior, but must be explicit and separately specified.

### 10. Security model

The default PR workflow runs with minimal permissions.

Recommended permissions:

```yaml
permissions:
  contents: read
  security-events: write # only if uploading SARIF
  actions: read
```

It must not need:

```yaml
pull-requests: write
contents: write
```

because it does not post comments or mutate source.

If future comment posting is added, it must use a separate trusted workflow:

```text
pull_request workflow:
  run analyzer with read-only permissions
  verify artifacts
  upload comment-plan

workflow_run / trusted workflow:
  download artifacts
  verify comment-plan again
  post or update comments
```

The trusted poster must not regenerate analyzer truth.

It consumes verified artifacts only.

### 11. CI proof

The spec is satisfied when the following pass:

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- check-pr

cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --out-dir target/unsafe-review-first-pr-smoke

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-first-pr-smoke
```

No-card proof:

```bash
cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/safe_code_no_cards \
  --diff fixtures/safe_code_no_cards/change.diff \
  --out-dir target/unsafe-review-no-card-smoke

cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-no-card-smoke
```

Policy report proof, when policy artifacts are included:

```bash
cargo run --locked -p unsafe-review -- policy report \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --format json

cargo run --locked -p unsafe-review -- policy report \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --format markdown
```

### 12. Final design summary

```text
The gate proves the packet is valid.
The packet reports unsafe-review evidence.
The evidence is advisory.
The reviewer gets a useful summary.
The comment plan stays quiet.
The IDE/LLM data is structured.
The tool never claims safety.
```

Not:

```text
unsafe found -> fail
```

But:

```text
unsafe-review packet malformed or dishonest -> fail
unsafe-review evidence found -> advise
```
