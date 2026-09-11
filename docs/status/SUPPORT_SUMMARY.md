# Support summary

Retained table evidence: 2026-08-30 (audited against swarm `origin/main` at
`c741f69d75e9fc9f590b638d37086b6e87d65e68`, inactive dependency ledger
`125de5f683286c4e8da04b76c6633a2a8e123f5a`)

CLI and compatibility wording refreshed 2026-09-07 against the release
documentation input `a78770cfabe5daf631e5829133a0cae8db0210d7`. This narrow
refresh does not rerun the table's checks, repin historical receipts, activate
a dependency freeze, or qualify a candidate.

This is the front panel for current `unsafe-review` support posture. The full
claim-to-proof ledger remains [`SUPPORT_TIERS.md`](SUPPORT_TIERS.md).
The current schema and availability receipt is
[`RELEASE_COMPATIBILITY.md`](RELEASE_COMPATIBILITY.md); it records the accepted
[SPEC-0011](../specs/UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md) producer/verifier
contract, including required and additive fields, closed vocabularies, the
producer floor, and deprecation boundaries. Review-kit remains schema `0.1`;
saved LSP is `0.2`, with legacy rendering separately bounded by SPEC-0012.
Accepted contract rules do not establish current-candidate consumer execution;
that qualification remains with #1921.

All surfaces describe static unsafe-review evidence. None of them is a
memory-safety proof, UB-free claim, Miri-clean claim, target-feature availability
proof, site-execution proof, or calibrated policy gate.

## Proof Vocabulary

| Label | Meaning |
|---|---|
| Fixture-backed | Covered by curated fixtures, goldens, unit tests, or e2e tests. |
| Dogfood-backed | Exercised on selected real crates or PR diffs with recorded limits. |
| Calibrated | Measured across a documented corpus with known false-positive and false-negative behavior. |

No current surface is calibrated. Blocking policy remains out of scope until
calibration exists and support tiers are explicitly promoted.

## Retained posture and evidence

| Surface | Current posture | Evidence | Not claimed |
|---|---|---|---|
| ReviewCard schema, identity, and core card slices | Experimental | Fixture-backed; selected analyzer rules are dogfood-backed | Stable schema compatibility, broad precision/recall, or safety |
| First-run CLI path: `doctor`, `pr` (with `first-pr` / `review` compatibility aliases), `explain`, `context`, and saved artifacts | Experimental | Fixture-backed CLI e2e coverage and current release-readiness proof; public v0.3.8 is the last published release | Proof, policy authority, source edits, witness execution, agent execution, or live editor integration |
| PR artifacts: review-kit manifest, bounded ReviewCard queue preview, cards JSON, PR summary, bounded GitHub summary, SARIF, comment-plan, witness-plan, receipt audit, manual-candidates JSON, manual repair queue sidecar, tokmd packet input sidecar, saved LSP JSON, and repair queue | Experimental | Fixture-backed and workflow-verified; advisory artifact loop is dogfoodable | Automatic comments, branch protection, witness execution, source edits, agent execution, repair success, rendered tokmd output, or policy gating |
| Saved LSP projection and agent packet | Experimental | Fixture-backed and e2e-covered read-only projections from `ReviewCard` | Live editor integration, agent execution, source edits, or repair success |
| Witness routes and saved-output receipt import | Experimental | Fixture-backed route table plus saved-output receipt adapters | Running Miri/cargo-careful/sanitizers/Loom/Kani, site reach, or witness success beyond imported receipt scope |
| Manual candidate ledger | Experimental | Fixture-backed import, explain/context, witness-plan, receipt audit, saved-outcome projection, oracle-map, proof-mode, fix-boundary, and PR-aperture preservation for advisory manual candidates | Analyzer discovery, proof, witness execution, site execution, repository safety, or policy authority |
| Repo posture, badge JSON, outcome comparison, and policy report | Experimental | Fixture-backed; outcome comparison has limited saved-snapshot dogfood | Safety badge, release-grade dashboard, default no-new-debt, or blocking policy |
| Real-crate dogfood measurement | Experimental | Twenty-two capped repo snapshots and twenty-three selected PR diffs across real crates | Calibrated rates, full audits, uncapped performance guarantees, or ecosystem-wide coverage |
| Live `unsafe-review lsp` server | Experimental | Unit- and smoke-covered read-only server (SPEC-0018); not packaged in the editor extension | Source edits, witness execution, comments, blocking policy, or editor client integration |
| MIR/nightly facts and editor client integration | Deferred or planned | Requires later ADR/spec and proof | Default dependency, support promise, or v0.x gate behavior |

## Public and candidate posture

The last public release is `v0.3.8` (2026-06-18). The next candidate is still
unfrozen: its version, candidate SHA, dependency freeze, and publication state
remain unset in the [draft release cutline](RELEASE_CUTLINE.md). Do not read
Swarm main integration as public availability or qualification.

The public `v0.3.8` first-use path is install → `doctor` → `pr` → reviewer
summary → `explain`/`context` or human review → named external verification.
That release has `baseline init` but no top-level `init`.

On unpublished swarm main, optional top-level `init` previews repository
adoption before `doctor` and `pr`; the integrated `pr` route presents the
bounded action-first front panel. Preview `init` applies no workflow or
configuration; explicit `--out` writes only the proposal JSON. It remains
separate from baseline creation. The [first-use guide](../FIRST_USE.md#preview-repository-adoption)
selects the workspace binary explicitly. These integrated commands retain
the existing experimental posture and require separate installed-candidate
qualification.

## Promotion Posture

- Fixture-backed surfaces may stay experimental until they survive dogfood.
- Dogfood-backed surfaces may describe the exact crates, PRs, and limits tested.
- Calibrated support requires measured outcomes, not just more fixtures or a
  larger support-tier table.
- Policy gating is not ready. `--policy no-new-debt` is explicit opt-in, and
  `policy report` is advisory-only.

## Core Boundary

`unsafe-review` finds unsafe Rust changes missing a safety contract, guard, test,
or witness. It routes reviewers to the cheapest credible next action; it does
not prove the repository safe.
