# Documentation map

| Layer | Owns | Path |
|---|---|---|
| Adoption front door | one-page newcomer routing to all five delivery surfaces | `docs/START-HERE.md` |
| Mission / vision | product purpose and end state | `docs/MISSION.md`, `docs/VISION.md` |
| Product principles | settled design values: restraint, advisory-by-design, evidence not proof, single truth, group-not-delete, naming-vs-doing, detector-discipline symmetry | `docs/PRINCIPLES.md` |
| Roadmap | release direction | `docs/ROADMAP.md` |
| Find/fix workflow | end-to-end UB-risk review seam workflow from first-pr through explain, agent packet, witness receipt, and outcome comparison | `docs/FIND_AND_FIX_UB.md` |
| First-use guide | install and first useful local run from crates.io | `docs/FIRST_USE.md` |
| First-hour guide | maintainer first-hour walkthrough from install to one credible review action | `docs/FIRST_HOUR.md` |
| CLI guide | current user-facing commands and artifact surfaces | `docs/CLI.md` |
| Repo style | evidence-machine doctrine, tool-role split, exception ownership, CI economics, and review-fast agent rule | `docs/REPO_STYLE.md` |
| CI economics and PR artifacts | advisory CI lanes, LEM budgeting, cost policy, ripr/unsafe-review lane boundaries, UB-risk review cookbook, first-pr packets, comment plans, and future trusted poster design | `docs/ci/cost-and-verification-policy.md`, `docs/ci/lem-budgeting.md`, `docs/ci/ripr.md`, `docs/ci/unsafe-review.md`, `docs/ci/UB_RISK_REVIEW_CI.md`, `docs/ci/PR_CI.md`, `docs/ci/COMMENT_PLAN_EXAMPLES.md`, `docs/ci/TRUSTED_COMMENT_POSTER.md` |
| GitHub Actions user guide | copy-paste drop-in workflow for downstream Rust repositories | `docs/ci/github-actions.md`, `.github/examples/unsafe-review-first-pr.yml` |
| Coverage telemetry | advisory cargo-llvm-cov + Codecov execution-surface signal, not unsafe correctness | `docs/ci/coverage.md`, `.github/workflows/coverage.yml` |
| Analysis design | analyzer evidence applicability and refactor rails | `docs/analysis/` |
| Explanations | concept notes, trust boundaries, reviewer examples, fix recipes, agent repair workflow, and agent packet examples | `docs/explanation/` |
| Editor workflow | saved read-only editor projection walkthrough | `docs/editor/saved-lsp-json.md` |
| Editor extension MVP | publishable saved-lsp viewer extension scope and non-goals | `docs/editor/extension-mvp.md` |
| Editor marketplace publication | owner pre-publication checklist and manual publish workflow | `docs/editor/marketplace-publication.md`, `.github/workflows/editor-publish.yml` |
| Proposals | why a workstream exists | `docs/proposals/` |
| Releases | release targets, readiness notes, publication receipts, and crates.io patch runbooks | `docs/releases/`, `docs/releases/CRATES_IO_PATCH_RELEASE.md` |
| Specs | behavior contracts | `docs/specs/` |
| ADRs | durable architecture decisions | `docs/adr/` |
| Templates | reusable proposal, spec, plan, closeout, swarm digest, and publication receipt skeletons | `docs/templates/` |
| Contribution process | swarm-to-main routing, PR disposition, source PR requirements, and source history catch-up | `docs/contributing/SWARM_TO_MAIN.md`, `docs/contributing/SOURCE_HISTORY_CATCHUP.md` |
| Spec style | durable source-of-truth ownership boundaries and artifact role split | `docs/spec-style.md` |
| Spec rails guide | contributor workflow for source-of-truth rails and tool-state boundaries | `docs/contributing/spec-rails.md` |
| Agent orchestration | portable multi-agent build doctrine: spine, model tiers, two orchestration modes, issue routing, hygiene, and adopt-in-your-lane template | `docs/contributing/AGENT-ORCHESTRATION.md` |
| Implementation plans | PR-sized sequence and proof commands | `plans/` |
| Active lane | current dogfood-calibrated evidence loop | `docs/status/DOGFOOD_CALIBRATED_EVIDENCE_LANE.md` |
| Dogfood index | selected real-crate dogfood targets, usefulness notes, and recorded outcome movement | `docs/dogfood/index.md`, `docs/dogfood/index.json`, `docs/dogfood/usefulness-notes.md` |
| Dogfood narrative | narrative summary of real-world findings across the seven dogfood crates: what fired, what did not, corrections, and open gaps | `docs/dogfood/REAL_WORLD_FINDINGS.md` |
| Agent integration guide | using unsafe-review with a coding agent: bounded-card model, packet fields, readiness routing, do-not-do rules, and receipt discipline | `docs/explanation/using-unsafe-review-with-agents.md` |
| Fuzzing | manual analyzer robustness harness and input contract | `docs/FUZZING.md` |
| Support summary | concise support posture front panel | `docs/status/SUPPORT_SUMMARY.md` |
| Support tiers | detailed product claim to proof ledger | `docs/status/SUPPORT_TIERS.md` |
| Objective audit | current objective evidence and remaining gaps | `docs/status/OBJECTIVE_AUDIT.md` |
| Policies | ledgers, baselines, suppressions | `policy/` |

Rule: do not make every document do every job. Proposals say why, specs say what,
ADRs say why this architecture, plans say how, and policies hold exceptions.
