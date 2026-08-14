# Repository lifecycle surface map

Status: inventory and migration map for issue
[#1923](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1923).
Audited against swarm commit
`d20b851312b5f5364b2087418efe3763894d99b3`.

This document names the owner of each repository lifecycle responsibility. It
does not introduce a new lifecycle, runtime adapter, agent role, command, hook,
or workflow. When this inventory conflicts with live repository state, the
precedence in `AGENTS.md` still applies.

## Authority boundaries

The repository protocol and a runtime implementation of that protocol are
different things:

- `AGENTS.md` owns the repository operating contract and source-of-truth
  precedence.
- `.allow`, linked specs and plans, and issue-linked work specs own durable
  contracts. They may describe zero, one, or many concurrent work items; they
  do not select the next task.
- GitHub issues and PRs own the live portfolio, dependencies, discussion, and
  disposition.
- Deterministic tests and gates own pass/fail. A model, agent role, label,
  assignee, current session, task list, or locally selected lane is never proof.
- Runtime adapters such as `CLAUDE.md` and `.claude/agents/*.md` may explain how
  one tool carries out the protocol. They do not redefine repository authority
  or promise that another runtime has the same models, tools, lifecycle, or
  messaging primitives.
- Handoffs and closeouts preserve evidence and decisions. Historical handoffs
  are not current scheduling state.

The disposition vocabulary in this map is:

- `REUSE`: retain the surface and its current responsibility.
- `THIN`: retain the surface, but replace duplicated detail with links to its
  named authority.
- `RENAME_OR_ALIAS`: retain the capability while presenting it through a
  runtime-neutral lifecycle name; the existing runtime name may remain an
  adapter alias.
- `DEPRECATE`: keep temporarily for compatibility while directing new use to a
  named replacement.
- `REMOVE_AFTER_REPLACEMENT`: remove only after the replacement exists and the
  stated proof has passed.

## Lifecycle transition map

| Transition | Trigger | Durable artifact owned | Actual authority | Primary consumers | Existing execution surfaces | Disposition |
|---|---|---|---|---|---|---|
| Prepare and research an issue | A user-selected issue/PR, or a bounded candidate selected from current GitHub state | Evidence-backed issue context; no repository-wide active-task record | GitHub issue/PR plus the source-of-truth precedence in `AGENTS.md` | Maintainer, planner, builder | `repo-preflight`, `issue-factcheck`, `source-divergence`, read-only Git/GitHub inspection | `RENAME_OR_ALIAS` the runtime roles as `preflight` and `issue-factcheck`; preserve manual inspection |
| Compile the accepted work contract | The premise is current and the proposed seam is review-forward | Issue-linked work spec, linked spec/ADR/plan where needed, acceptance IDs, proof commands, non-goals, risk, rollback, claim boundary | SPEC-0044 defines the contract, `docs/schemas/issue-work-spec.schema.json` defines its version-one shape, and the issue plus live GitHub disposition supplies current context; `.allow` registers graph visibility but is not a scheduler | Builder, reviewer, PR author | `xtask check-work-specs`, `plan-refuter`, spec/doc gates | `REUSE` the work-spec contract and checker; `RENAME_OR_ALIAS` the refuter as a runtime-neutral plan review |
| Admit or resume one writer | The contract is verified, prerequisites are satisfied, and no live writer owns the branch | Branch/worktree identity and a bounded handoff; no durable scheduler record | Live branch/worktree/PR state and controller decision within the accepted contract | Writer and coordinator | Git/worktree inspection, runtime implementer adapter | `RENAME_OR_ALIAS` the implementer adapter; do not encode session ownership in `.allow` |
| Build and prove | A writer receives an accepted contract | Scoped commit, tests, generated artifacts or receipts required by the contract | Code/spec/policy plus deterministic proof commands | Reviewer, CI, maintainers | Targeted tests, `check-local`, artifact validators, `check-pr` | `REUSE`; `check-local` stays partial and `check-pr` stays the comprehensive local gate |
| Review the exact head | A scoped commit or PR head exists | Review findings tied to exact base/head and artifacts | Actual diff, controlling contract, tests, hosted checks, and repository policy | Writer, maintainer, merge controller | Claim scan, artifact verification, focused review, GitHub checks | `RENAME_OR_ALIAS` runtime reviewer roles; no agent verdict becomes authority |
| Respond to feedback | A current-head review or hosted check has an actionable finding | New scoped commit or an evidence-backed disposition reply | Current review thread/check plus the accepted contract | Writer, reviewer, maintainer | Warm writer callback where supported; manual patch/review loop everywhere else | `REUSE` the loop, but keep messaging APIs and warm-context economics as runtime examples |
| Publish or merge | Exact head is scoped, reviewed, green, and allowed | PR body, hosted check record, merge commit, or source-owned publication receipt | GitHub merge state and repository policy; source repository owns release/publication | Maintainers, downstream source-promotion work | PR template, hosted CI, normal squash merge | `REUSE`; release, tagging, deployment, and source promotion remain separately authorized |
| Reconcile and clean | Merge or explicit disposition is confirmed | Issue/PR closeout, follow-up issue where needed, clean lane-owned worktree/branch state | Merged `origin/main`, GitHub disposition, closeout/receipt artifacts | Maintainers and future agents | `cleanup-audit`, Git/worktree checks, post-merge proof | `REUSE` the advisory audit; deletion stays an explicit controller action limited to known lane-owned residue |

The transitions are ordered for one work item, but they are not a repository
stage machine. Multiple issues may be at different transitions concurrently,
and zero admitted writers is valid.

## Surface inventory and disposition

### Root and contributor guidance

| Surface | Trigger and owned artifact | Consumers | Overlap or known drift | Disposition and replacement proof |
|---|---|---|---|---|
| `AGENTS.md` | Every repository operation; owns the operating contract, precedence, product boundary, and lane rules | All contributors and runtime adapters | Contains detailed model/provider examples and lifecycle explanation also present in the orchestration guide | `THIN`: retain normative authority and repo-specific boundaries; future thinning may link explanatory economics to this map and `AGENT-ORCHESTRATION.md`. Proof: doc gates plus a review that every removed rule remains reachable from the root router |
| `CLAUDE.md` | Claude Code entry; owns no durable repository state | Claude Code sessions | Repeats product, command, architecture, and routing doctrine from `AGENTS.md` and specs | `THIN`: retain runtime startup guidance and links; do not make its commands or capability claims repository authority. Proof: Claude entry still routes to `AGENTS.md`, source truth, build gate, and trust boundary |
| `docs/contributing/AGENT-ORCHESTRATION.md` | Detailed orchestration explanation and reusable examples | Maintainers and adapter authors | Fixed Haiku/Sonnet/Opus tiers, specific background-agent messaging, and token-cache figures are runtime-specific examples presented beside durable protocol | `REUSE`: keep as the detailed doctrine, but future edits must label model names, cache economics, and messaging APIs as examples. This map owns the disposition inventory, not a second lifecycle |
| `docs/README.md` | Documentation discovery | Contributors and doc gates | Navigation only; it must not become lifecycle authority | `REUSE` as the index linking this map and the detailed orchestration guide |
| `.github/PULL_REQUEST_TEMPLATE.md` | PR creation; owns the review-forward reporting shape | PR authors and reviewers | Some command/checklist guidance repeats root docs, intentionally close to delivery | `REUSE`: the template remains the delivery checklist and claim-boundary prompt; it does not decide readiness by itself |
| `docs/handoffs/` and closeout artifacts | A completed, blocked, or transferred lane needs durable evidence | Maintainers and future agents | Historical records can look like current routing if read without live GitHub checks | `REUSE`: retain as evidence/history; always reconcile with current refs, issues, and PRs before action |

### Source routing and history surfaces

These three runbooks are directional. Their different merge models are part of
their contracts, not interchangeable Git preferences.

| Surface | Trigger | Owner and actual authority | Consumers | Direction and merge model | Relationship to `source-divergence` | Disposition and manual rollback |
|---|---|---|---|---|---|---|
| `docs/contributing/SWARM_TO_MAIN.md` | Any routine cross-repository work selection, source PR disposition, or promotion eligibility decision | Repository maintainers; owns the default routing and disposition policy under the `AGENTS.md` repository-role contract | Swarm contributors, source maintainers, PR reviewers | Routine development starts in swarm and curated work moves swarm to source through narrow source PRs; exceptional history repair delegates to `SOURCE_HISTORY_CATCHUP.md`, while the reverse release mirror delegates to `SWARM_MIRROR.md` | Requires the advisory report before routine work and interprets `new_source_commits`; it does not generate or replace the report | `REUSE`. Manual rollback: stop before cross-repo mutation, leave an evidence-backed handoff, and abandon or revert only the lane-owned unmerged branch; never rewrite either `main` |
| `docs/contributing/SOURCE_HISTORY_CATCHUP.md` | Source is materially behind swarm, is missing product-relevant reviewed state, or must regain swarm ancestry before release | Source maintainers and explicit release/source authority; owns the exceptional history-repair procedure | Source PR author, reviewer, release maintainer | Swarm to source via a real `--no-ff` merge commit, never squash or rebase; routine narrow promotion must not use this runbook | Divergence is preflight evidence only. The runbook additionally requires ancestry, parent, conflict, source-suite, artifact, and remaining-tree-diff proof | `REUSE`. Manual rollback: abort an in-progress merge or close the unmerged catch-up PR and leave source `main` unchanged; after merge, use an explicit corrective PR rather than history rewrite |
| `docs/contributing/SWARM_MIRROR.md` | A source-owned release is already published and source is ahead of the workbench | Swarm maintainers; owns release-metadata absorption and the `policy/source-sync.toml` checkpoint update, not publication | Swarm sync author, reviewers, later routine developers | Source to swarm via a normal squash PR; it deliberately does not import source ancestry | Uses `source-divergence` / `check-source-sync` as the before/after checkpoint test and expects `new_source_commits=0` after absorption | `REUSE`. Manual rollback: stop or close the unmerged mirror branch; if an incorrect mirror merged, repair metadata/checkpoint through a new reviewed swarm PR rather than resetting history |

None of these runbooks authorizes release publication. Their manual paths use
ordinary Git and GitHub operations and remain available when a runtime adapter
or helper is absent.

### Compile-contract surfaces

| Surface | Trigger and owned artifact | Authority and consumers | Overlap or known drift | Disposition |
|---|---|---|---|---|
| `docs/specs/UNSAFE-REVIEW-SPEC-0044-issue-linked-work-specs.md` | Creating or changing an issue-linked work contract; owns the contract semantics and authority boundary | `repo-infra` owner; controller, builder, reviewer, verifier, and closeout consumers | Status is `proposed`; its version-one contract is made concrete by the schema and checker. Generic `.allow` registration alone is insufficient because cargo-allow has no first-class `work_spec` kind | `REUSE`; change semantics through the spec and linked governance review, not through an adapter |
| `docs/schemas/issue-work-spec.schema.json` | Authoring or validating a version-one work spec; owns the machine-readable field shape | SPEC-0044 is the design authority; authoring tools and validators consume the schema | JSON Schema shape and the Rust checker must remain aligned; neither executes proof or consults GitHub | `REUSE` |
| `plans/work-specs/examples/UNSAFE-REVIEW-WORK-1900.toml` | A contributor needs the canonical minimal version-one example | SPEC-0044 and the schema govern it; planners, adapters, fixtures, and reviewers consume it | Registered as a draft `plan_item` only for cargo-allow compatibility; it is not a default goal or scheduler | `REUSE` as the canonical example; update it with schema/spec changes and keep `check-work-specs` green |
| `xtask/src/work_specs.rs` / `xtask check-work-specs` | A work-spec/schema/example change enters local or CI proof | Owns offline structural enforcement of the version-one contract; contributors and CI consume its verdict | Mirrors the schema in executable checks, including rejection of scheduling fields; validates structure rather than live issue truth | `REUSE`; schema/checker drift must fail focused tests or the gate |

### Runtime role adapters

Every file below is an executable Claude-oriented adapter, not a new source of
repository truth. The durable artifact column therefore names what the role may
inspect or help produce; the accepted issue/spec and deterministic tools still
own the contract.

| Surface | Trigger | Durable artifact involved | Consumers | Overlap or known drift | Disposition and replacement proof |
|---|---|---|---|---|---|
| `.claude/agents/repo-preflight.md` | Before non-trivial work | Read-only evidence packet about refs, worktrees, portfolio, and source sync | Coordinator | Still directs readers through `.allow/goals/active.toml` as a controlling lane, which can be mistaken for a scheduler | `RENAME_OR_ALIAS` to runtime-neutral `preflight`; replace the active-goal instruction with `.allow` graph plus selected GitHub contract. Proof: zero/one/many work items remain valid and no task is auto-selected |
| `.claude/agents/issue-factcheck.md` | Before assigning a writer | Premise/buildability evidence packet | Coordinator and planner | Correctly separates issue validity from repository preflight; model/tool declarations are Claude-specific | `RENAME_OR_ALIAS` to `issue-factcheck`; preserve the explicit current-code checks and evidence format |
| `.claude/agents/plan-refuter.md` | After drafting a plan, before implementation | Refutations tied to paths, commands, acceptance, and cleanup | Planner and coordinator | Also names `.allow/goals/active.toml` as a controlling artifact; overlaps fact-check only where stale assumptions are relevant | `RENAME_OR_ALIAS` to `plan-review`; use the selected work contract and graph without a singleton scheduler. Proof: stale-path, command, acceptance, scope, and cleanup checks remain |
| `.claude/agents/implementer.md` | After writer admission | Scoped commit and proof packet | Coordinator and reviewer | Uses the legacy active-goal read order; pins a provider model and assumes isolated-agent tooling | `RENAME_OR_ALIAS` to `writer`; retain isolation, scope, proof, commit, and report requirements while making model/tool choice adapter-local |
| `.claude/agents/claim-boundary.md` | User-facing wording, output, spec, PR, or release review | Read-only forbidden-claim findings | Writer, reviewer, release maintainer | Project-specific and intentionally overlaps deterministic claim gates as an independent semantic scan | `RENAME_OR_ALIAS` to `claim-review`; deterministic gates outrank its verdict |
| `.claude/agents/artifact-verifier.md` | Generated advisory artifact review | Artifact-contract evidence packet | Writer and reviewer | Correctly defers to deterministic validators; assumes named Claude tools | `RENAME_OR_ALIAS` to `artifact-review`; preserve no-patch/no-regenerate boundary |
| `.claude/agents/ci-log-triage.md` | Large hosted or local failure log | Bounded failure classification | Writer and coordinator | Runtime role is useful; its closed vocabulary is advisory and cannot replace the check result | `RENAME_OR_ALIAS` to `ci-triage`; preserve bounded excerpts and no-rerun/no-push boundary |
| `.claude/agents/cleanup-auditor.md` | End of a task/session | Read-only cleanup candidates with ownership class | Coordinator | Name overlaps `xtask cleanup-audit`, but the role additionally classifies branches, artifacts, and processes; neither may delete | `RENAME_OR_ALIAS` to `cleanup-review`; keep `cleanup-audit` as the deterministic advisory command. Proof: uncertain/user-owned residue can never be classified safe by default |

Runtime adapters may pin a model or enumerate tools because their host requires
that configuration. Such pins describe one implementation, not a repository
requirement. Other runtimes may use one agent, several agents, manual review, or
different tools as long as they preserve the authority, isolation, proof, and
claim boundaries above.

### Repository commands and validators

| Surface | Lifecycle trigger | Artifact or signal owned | Consumers | Overlap or limitation | Disposition |
|---|---|---|---|---|---|
| `cargo run --locked -p xtask -- source-divergence` | Preflight and post-merge source/swarm reconciliation | Advisory divergence report | Coordinator and source-promotion maintainer | Networked/read-only and not merge proof | `REUSE` |
| `cargo-allow doctor/check/worklist --profile spec-system` | Durable graph inspection | Structural configuration/audit/worklist output | Planner and coordinator | Availability is environment-specific; worklist is not priority or scheduling | `REUSE` when installed; absence must be reported, not replaced with invented schema |
| `cargo run --locked -p xtask -- check-work-specs` | Contract compilation or change | Offline validation of the SPEC-0044/schema contract: issue URL, scope, invariants, acceptance/proof, integration, risk, rollback, and no scheduling fields | Planner, writer, CI | Validates shape, not factual correctness or live issue state; cargo-allow registration supplies graph visibility only | `REUSE` |
| `cargo run --locked -p xtask -- check-spec-status` | Spec or command-reference change | Spec-status consistency result | Contributors and CI | Does not establish implementation correctness | `REUSE` |
| `cargo run --locked -p xtask -- check-docs` | Documentation change | Required-doc and wording result | Contributors and CI | Does not validate every external link or runtime claim | `REUSE` |
| `cargo run --locked -p xtask -- check-doc-artifacts` | Governed doc-artifact change | Doc-artifact ledger consistency result | Contributors and CI | Checks registered surfaces, not live GitHub state | `REUSE` |
| `cargo run --locked -p xtask -- check-local` | Shift-left proof before the full gate | Machine-readable partial-proof receipt under `target/check-local/` | Writer | Diff-selected and explicitly not merge readiness | `REUSE` |
| Targeted crate/fixture tests | Build and improve loop | Test result for the named seam | Writer and reviewer | Proves only exercised cases | `REUSE` |
| `check-advisory-artifacts` / `check-first-pr-artifacts` | Artifact-producing changes | Deterministic artifact-contract verdict | Writer and artifact reviewer | Validates supplied bundles; does not run the analyzer or prove safety | `REUSE` |
| `cargo run --locked -p xtask -- check-pr` | Before push/merge and after relevant integration | Comprehensive repository gate result | Writer, reviewer, CI | Does not include fmt or Clippy and is not release readiness | `REUSE` |
| `cargo fmt --all --check`, workspace Clippy, and workspace tests | Build and hosted core gate | Formatting, lint, and test results | Writer and CI | Each is one rung, not a merge verdict alone | `REUSE` |
| Hosted required checks and exact-head review | PR convergence | GitHub check/review state tied to the PR head | Merge controller | Local green cannot substitute; skipped advisory lanes are not passes | `REUSE` |
| `cargo run --locked -p xtask -- cleanup-audit` | Reconciliation and cleanup | Advisory disk/worktree report | Coordinator | Reports only; does not know complete provenance and never deletes | `REUSE` |
| `cargo run --locked -p xtask -- check-goals` and editable `.rails` routing | Parity window only | Legacy compatibility result | Migration maintainers | `.rails` is not current scheduling or durable authority | `DEPRECATE`; remove only after the `.allow` replacement and parity retirement are proven by the governing migration issue |

Raw Git, GitHub, Cargo, and shell commands remain valid manual paths. Runtime
wrappers may standardize invocation, but wrappers do not change which artifact
or check owns the result.

## Drift and overlap findings

1. The runtime role files `repo-preflight`, `plan-refuter`, and `implementer`
   still route through `.allow/goals/active.toml`. The repository contract now
   treats GitHub as the concurrent portfolio and `.allow` as a non-scheduling
   durable graph. A later adapter PR should replace only those routing phrases;
   this inventory does not change the adapter files.
2. `AGENTS.md` and `AGENT-ORCHESTRATION.md` describe fixed model tiers, while
   model names, costs, cache behavior, and tool availability are runtime
   capabilities. The durable rule is capability-based: bounded discovery and
   verification, an appropriately capable writer, independent review, and a
   deterministic floor. Provider/model names are examples.
3. `CLAUDE.md` repeats repository doctrine as a convenience summary. Its own
   conflict rule already points back to `AGENTS.md`; future edits should reduce
   duplicated normative prose rather than create another authority.
4. The `cleanup-auditor` role and `cleanup-audit` command have similar names but
   different coverage. The command reports deterministic disk/worktree facts;
   the role adds an advisory provenance classification. Neither authorizes
   deletion.
5. The orchestration guide presents a fixed issue-file/spec/build sequence.
   Existing accepted contracts may already contain the issue, spec, or plan, so
   the durable requirement is to verify and reuse the current artifact rather
   than recreate every stage ceremonially.

These findings are dispositions, not authorization to edit runtime adapters or
root instructions in this docs-only slice.

## Smallest progressive-disclosure surface

The intended steady state has five layers:

1. **Root router:** `AGENTS.md` states repository authority, product boundaries,
   required preflight, worktree safety, proof/merge rules, and links onward.
2. **Lifecycle map:** this document says which existing surface owns each
   transition and which overlaps should be thinned.
3. **Detailed doctrine:** `AGENT-ORCHESTRATION.md` explains orchestration,
   verification, and economics, with runtime-specific examples labeled as such.
4. **Accepted work packet:** the selected GitHub issue plus a SPEC-0044/schema
   conforming work spec and its `.allow`-linked spec/plan carries objective,
   scope, acceptance, proof, non-goals, risk, rollback, and claim boundary for
   one lane. `UNSAFE-REVIEW-WORK-1900.toml` is the canonical version-one
   example; `.allow` registration is graph visibility, not contract semantics.
5. **Runtime adapter:** `CLAUDE.md`, `.claude/agents/*`, a Codex adapter, or a
   manual checklist translates the protocol into available runtime operations.

No layer mirrors the whole layer above it. A runtime adapter links to durable
contracts; it does not copy the portfolio, persist current session state, or
declare itself proof.

## Removal and rollback rules

- Do not remove a root rule until its replacement is linked, discoverable, and
  passes `check-docs`, `check-doc-artifacts`, and `check-spec-status` where
  applicable.
- Do not retire `.rails` compatibility checks until the governing migration has
  recorded bounded parity and the `.allow` replacement is authoritative.
- Do not rename a runtime role without an alias or coordinated consumer update.
- Manual issue inspection, isolated Git worktrees, direct proof commands,
  GitHub review, and explicit cleanup remain supported rollback routes if an
  agent/config/skill adapter is absent or broken.
- Reverting this document and its documentation-map link fully rolls back this
  inventory; it changes no runtime or product behavior.

## Claim boundary

This map establishes ownership and proposed disposition for the inspected
lifecycle surfaces. It does not implement packet schemas, runtime configuration,
skills, hooks, commands, workflows, product behavior, or publication. It does
not prove an implementation correct, establish merge or release readiness, or
make a safety, UB-free, Miri-clean, site-execution, or calibrated-accuracy claim.
