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
target/unsafe-review/review-kit.json
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/github-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/receipt-audit.md
target/unsafe-review/manual-candidates.json
target/unsafe-review/manual-repair-queue.json
target/unsafe-review/tokmd-packets.json
target/unsafe-review/usefulness-telemetry.json
target/unsafe-review/lsp.json
target/unsafe-review/repair-queue.json
```

The existing first-run lane already identifies this bundle and verifier as the public first-run cockpit surface.

The workflow should add a concise GitHub job summary, but should not post inline comments by default.

### 3. Required artifacts

#### 3.0 `review-kit.json`

Review-kit manifest and discovery index for the first-pr artifact bundle.

Requirements:

```text
must parse as JSON
must include schema_version = 0.1
must include tool and tool_version
must include source = first_pr
must include policy = advisory
must include scope matching cards.json
must include base/head metadata when known
must include changed-file, changed-Rust-file, changed-non-Rust-file, card, and
  open-actionable-gap counts matching cards.json
must include top_card_id, or null only when no cards exist
must include copy-only handoff commands for the reviewer summary, receipt audit,
  and top-card explain/context commands when a top card exists
must include a bounded ReviewCard queue preview under handoff.review_cards,
  with cards.json and repair-queue.json artifact references, queue limit,
  omitted-card count, and entries projected from known ReviewCards only
must keep handoff.review_cards entries aligned with cards.json identity,
  location, operation, missing evidence, next action, verify commands, witness
  routes, and with repair-queue.json bucket, bucket-reason, and agent-readiness
  state
must include manual candidate handoff metadata pointing to
  manual-candidates.json and manual-repair-queue.json, with
  analyzer_discovered = 0 and copy-only explain/context/witness-plan commands,
  a bounded candidate_queue, omitted count, and implementer handoff cues when
  manual candidates exist
must include handoff.repair_queues as a side-by-side front panel with
  ReviewCard repair-queue.json counts and manual-repair-queue.json counts,
  without merging their sources
must list every required first-pr artifact with relative paths
must include artifact kind, format, and schema_version/null metadata
must include trust boundary wording
```

The manifest is a discovery projection. It must not reclassify ReviewCards or
create a second source of truth for operation family, obligation, evidence,
witness route, repair bucket, outcome, or policy posture.

The ReviewCard queue preview must not include manual candidates. Manual
candidates remain under `manual-candidates.json` and
`handoff.manual_candidates.candidate_queue`. The ReviewCard queue is copy-only
and advisory; `repair-queue.json` remains the checked aggregate queue truth.
ReviewCard queue entries project `verify_commands` and full `witness_routes`
route objects from `cards.json`; they are reviewer handoff cues only and do not
claim witness execution.

`handoff.repair_queues` may place the checked ReviewCard repair queue and the
manual-candidate repair queue side by side for reviewer and agent routing. It
must cross-check ReviewCard counts, bucket counts, and agent-ready counts
against `repair-queue.json`, and manual candidate queued counts against
`manual-repair-queue.json`. It must not merge manual candidates into
`repair-queue.json`, create a new repair vocabulary, run agents, run witnesses,
edit source, post comments, claim repair success, or claim policy readiness.

Manual candidate markers (`source = manual`, `manual_candidate`, or
`analyzer_discovered`) must not appear in ReviewCard-only first-pr artifacts:
`cards.json`, `cards.sarif`, `comment-plan.json`, `lsp.json`,
`repair-queue.json`, `policy-report.json`, or `policy-report.md`. The artifact
verifier rejects marker leakage instead of silently converting manual
candidates into analyzer output.
`manual-candidates.json`, `manual-repair-queue.json`, and
`handoff.manual_candidates` are the manual-candidate sidecars allowed to carry
manual markers. `manual-candidates.json` and `handoff.manual_candidates` must
carry structured `reviewcard_artifact_applicability` metadata that records
ReviewCard-only surfaces as `reviewcard_only`, including explicit
`policy-report.json` and `policy-report.md` entries, and sets manual-candidate
applicability and marker allowance to false.

The handoff commands are reviewer and agent discovery aids only. They must not
imply that unsafe-review ran witnesses, ran an agent, posted comments, edited
source, or enforced blocking policy.

#### 3.1 `cards.json`

Canonical machine-readable ReviewCards.

Requirements:

```text
must parse as JSON
must include schema_version = 0.2
must include tool_version
must include a provenance block
must include trust boundary
must include stable ReviewCard identities
must include operation family
must include hazards
must include obligation evidence
must include missing evidence
must include witness routes when available
must include confirmation_cue with hypothesis_to_confirm, build_this_first,
minimal_repro, confirmation_step, and trust_boundary
witness routes must keep `required = false` in the default advisory PR packet
```

`confirmation_cue` is a plan-only projection from each ReviewCard. It frames
the card as a static hypothesis and names the first build/run or witness-route
cue, but it must not imply that unsafe-review executed the cue, observed runtime
behavior, proved site execution, proved UB, or proved repository safety.

All other PR artifacts are projections from this card set.

##### 3.1.1 Provenance block (schema 0.2)

Schema 0.2 adds top-level `tool_version` and a nested `provenance` object to
`cards.json` and to `check`/`repo` `--format json` output. The bump is
additive: every schema 0.1 field is unchanged, and `schema_version` is the
discriminator consumers route on. Consumers that accepted 0.1 should accept
0.2.

```text
tool_version           always present; the unsafe-review semver
provenance.tool_version always present; same value, for consumers that parse
                         the provenance block exclusively
provenance.generated_at always present; RFC3339 UTC timestamp of artifact
                         generation
provenance.root_abs     resolved absolute workspace root; omitted when path
                         resolution fails (the existing relative `root` field
                         is unchanged)
provenance.base_sha     resolved base commit SHA in --base mode; omitted when
                         --base was not supplied or git resolution fails
provenance.head_sha     resolved HEAD commit SHA in --base mode; omitted under
                         the same conditions as base_sha
provenance.diff_path    diff file path in --diff <file> mode; omitted otherwise
provenance.diff_sha256  SHA-256 hex digest of the diff file content in
                         --diff <file> mode; omitted when the file is unreadable
provenance.dirty_worktree true when `git status --porcelain` reports
                         uncommitted changes, false when clean; omitted when
                         git is unavailable or the root is not a repository
```

Unavailable fields are omitted from the JSON object rather than emitted as
null; presence is the availability signal. The provenance block is traceable
evidence metadata, not proof: it identifies the inputs used to produce the
artifact so two runs against different inputs cannot emit byte-identical clean
receipts, but it does not prove correctness, input integrity against a
motivated attacker, or memory safety.

Partial/interim repo reports (the `.partial` report written during a repo scan
and on interrupt) still emit schema 0.1 without a provenance block, pending the
SPEC-0035 partial-status reconciliation in the next lane slice. Final reports
and `cards.json` always emit 0.2.

#### 3.2 `pr-summary.md`

Reviewer-facing front panel.

Must include:

```text
card count
top actionable card, when present
  static hypothesis to confirm, with no observed-runtime-behavior claim until
  external evidence confirms the route
  first confirmation step from the selected verify command or witness route
  explain handoff command
  bounded agent context handoff command
  agent handoff readiness, repair buckets, bucket reasons, and readiness reasons
  projected from `repair-queue.json`
compact card table
  rows project ReviewCard id, class, location, operation family, operation,
  missing evidence, primary route, and next action
missing evidence summary
witness route summary
  rows project ReviewCard id, primary route, route reason, and route command
  when available; commandless routes stay explicit manual review routes
receipt audit cue pointing to `receipt-audit.md`
  cue must say saved receipt metadata only and that no witness was run
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

#### 3.3 `github-summary.md`

Bounded GitHub job-summary fragment.

Must include:

```text
card count
top actionable card, when present:
  card id
  class
  location
  operation
  operation family
  static hypothesis to confirm
  missing evidence
  primary witness route, when present
  primary witness route command, when available
  next action
  first confirmation step
  explain handoff command
  bounded agent context handoff command
full bundle artifact list
review-kit manifest path
trust boundary
```

Must stay concise enough for CI display and must not duplicate the full
`pr-summary.md` card table, witness plan, or reviewer front panel.

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

#### 3.4 `cards.sarif`

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
SARIF rule IDs are the stable `ReviewClass` string values; changing a rule ID is
a code-scanning baseline contract change, not a wording-only edit.

#### 3.5 `comment-plan.json`

Plan-only inline-comment artifact.

Must include `schema_version = 0.1`.

Must include:

```text
mode = plan_only
policy = advisory
summary
comments[]
trust_boundary
```

May include:

```text
not_selected[]
```

`summary` must include:

```text
selected_count
not_selected_count
budget
reason
reason_code
```

These fields describe the bounded reviewer-noise budget. They do not create a
blocking policy or a second classification truth.

Each comment candidate must include:

```text
card_id
path
line
changed_line
class
priority
confidence
hypothesis_to_confirm
operation
operation_family
next_action
witness_routes
verify_commands
build_this_first
minimal_repro
confirmation_step
selection_reason
selection_reason_code
actionability
relevance
body
trust_boundary
```

`line` must be one-based and nonzero.

`build_this_first` must be a plan-only cue object projected from the first
verify command when available, otherwise from the first witness route or human
review path. It must not imply that unsafe-review ran the command or observed
runtime behavior.

`minimal_repro` must be a plan-only cue object projected from the first verify
command when available, otherwise from the first witness route or human review
path. It must carry a limitation that unsafe-review did not run the cue, observe
runtime behavior, prove site execution, prove UB, or prove repository safety.

Planned comments must not repeat a `card_id` or a `path`/`line` inline anchor.
Planned comments also must not repeat an `operation_family` plus
missing-obligation set by default; later cards with the same family/obligation
budget key remain visible in `not_selected[]` with an explicit budget reason.

Each `not_selected` entry must reference a known card, must not repeat a
planned comment card, and must include the ReviewCard operation, operation
family, next action, actionability, relevance, `changed_line`, and a reason for
staying out of the inline comment budget. Entries must also include a
machine-readable `reason_code` so agents do not need to parse prose. Entries
with `changed_line = false` must use reason `outside changed hunk` and
`reason_code = outside_changed_hunk`.

Review-budget reason codes are a closed vocabulary:

```text
bounded_reviewer_noise
top_actionable_card
outside_changed_hunk
human_deep_review_only
lower_relevance
covered_by_selected_family_obligation
covered_by_specific_operation_card
budget_exhausted
not_selected_by_policy
```

Every ReviewCard must be accounted for by either `comments[]` or
`not_selected[]`. A card may be absent from inline comments, but it must not be
absent from the comment-plan projection.

`summary.selected_count` must match `comments[]`,
`summary.not_selected_count` must match `not_selected[]`, and
`summary.budget` must match the hard inline comment budget.

Each body must include the plan-only trust boundary: artifact-only candidate,
unsafe-review did not post the comment, did not run witnesses, and did not make
a policy decision.

Each body must also project the referenced ReviewCard's class, operation,
operation family, missing-evidence summary, next action, first witness route
when present, and first verify command when present. The body is a concise
reviewer note, not a second source of truth.

Each body must stay at or below 220 words.

#### 3.6 `witness-plan.md`

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

#### 3.7 `receipt-audit.md`

Reviewer-readable saved witness receipt metadata audit.

Must include:

```text
summary counts
reviewer front panel
receipt quality buckets
trust boundary
```

The artifact is a static audit of saved receipt metadata against current
ReviewCards. It may report matched, unmatched, stale, expired, wrong identity,
wrong tool, weaker-than-route, command-hash mismatch, duplicate, or invalid
receipt metadata.

Matched receipts improve witness evidence only. They must not erase missing
contract, guard, or reach evidence, and they must not claim witness execution,
site execution, proof, UB-free status, Miri-clean status, source edits,
comments, or blocking policy.

#### 3.8 `lsp.json`

Saved editor/LLM projection.

Must include `schema_version = 0.1`.

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

Code actions must be command-only. They must not include `WorkspaceEdit` or
source-edit fields in the action or nested payloads.
`copyWitnessCommand` actions must copy only a command already projected from the
same ReviewCard's verify commands.

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

#### 3.9 `repair-queue.json`

Aggregate copy-only agent handoff queue.

Must include:

```text
schema_version = 0.1
mode = aggregate_repair_queue
source = review_card
policy = advisory
trust_boundary
summary counts
bucketed card entries
```

Allowed buckets:

```text
repairable_by_guard
repairable_by_safety_docs
repairable_by_test
requires_witness_receipt
requires_human_review
do_not_auto_repair
```

Each entry must reference a known ReviewCard and project:

```text
card_id
class
priority
confidence
operation_family
operation
path
line
missing_evidence
agent_readiness
bucket_reason
context_command
do_not_do
trust_boundary
```

`agent_readiness.state` must use the closed vocabulary `ready_for_agent`,
`requires_human_review`, `requires_witness_receipt`, or `unsupported`, and it
must agree with `agent_readiness.ready`: `ready = true` requires
`ready_for_agent`, and `ready = false` requires any other closed state.
`agent_readiness.reasons` must contain at least one explanation. Empty reasons
fail artifact verification because a queue entry without a readiness rationale
is not a bounded work order.

The same `card_id` must not repeat within one repair queue bucket. The same card
may appear in multiple buckets only when the bucket reasons are distinct and
card-scoped.
Entries in `requires_human_review` and `do_not_auto_repair` must not be marked
agent-ready.

No other bucket names are valid. Unknown repair queue buckets fail artifact
verification instead of creating a second, unchecked agent-task vocabulary.

`context_command` must be exactly:

```text
unsafe-review context <card-id> --json
```

`pr-summary.md` may surface the top card's aggregate queue state as a reviewer
cue, but that line is still a projection of `repair-queue.json`; it must not
invent a second agent-readiness truth.

The queue must not run agents, edit source, post comments, execute witnesses,
suppress cards, resolve cards, or claim proof, UB-free status, Miri-clean
status, site execution, calibrated precision/recall, or policy readiness.

#### 3.9a `manual-repair-queue.json`

Copy-only manual candidate repair handoff queue. This is a manual-candidate
sidecar, not the ReviewCard repair queue.

Must include:

```text
schema_version = manual-repair-queue/v1
mode = manual_candidate_repair_queue
source = manual_candidate
policy = advisory
summary counts aligned with manual-candidates.json
optional stable-byte seed source/count metadata and per-entry seed rows joined
  by manual candidate ID only
queue entries preserving source = manual, manual_candidate = true, and
  analyzer_discovered = false
copy-only explain/context/witness-plan commands
trust boundary wording stating not analyzer-discovered, not automatic repair,
  not proof, no agents, no witnesses, no source edits, no comments, and no
  blocking policy
```

The verifier must cross-check queue length, order, guidance, implementer
handoff, and summary maps against `manual-candidates.json`. It must not accept
the manual queue as a ReviewCard source or as repair execution evidence.

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
no operation_family unknown
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
operation_family unknown cards
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
- `target/unsafe-review/review-kit.json`
- `target/unsafe-review/pr-summary.md`
- `target/unsafe-review/github-summary.md`
- `target/unsafe-review/witness-plan.md`
- `target/unsafe-review/receipt-audit.md` (saved receipt metadata only; no witness was run)
- `target/unsafe-review/manual-candidates.json` (manual/advisory candidates, separate from ReviewCards)
- `target/unsafe-review/manual-repair-queue.json` (copy-only manual candidate handoff; no agent was run)
- `target/unsafe-review/tokmd-packets.json` (formatting input only; tokmd was not run)
- `target/unsafe-review/usefulness-telemetry.json` (operational diagnostic usefulness only; not calibrated precision/recall)
- `target/unsafe-review/repair-queue.json` (copy-only; no agent was run)

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

The first-pr artifact verifier scans every required bundle artifact for positive
overclaim wording, including `review-kit.json`, `cards.json`, `pr-summary.md`,
`github-summary.md`, `cards.sarif`, `comment-plan.json`, `witness-plan.md`,
`receipt-audit.md`, `manual-candidates.json`, `manual-repair-queue.json`,
`tokmd-packets.json`,
`usefulness-telemetry.json`, `lsp.json`, and `repair-queue.json`.

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

The first-pr verifier also checks bundled `policy-report.json` and
`policy-report.md` artifacts as ReviewCard-only policy-report projections and
rejects manual-candidate marker leakage in both files.

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
