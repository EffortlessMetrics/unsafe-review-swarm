# Documentation map

| Layer | Owns | Path |
|---|---|---|
| Mission / vision | product purpose and end state | `docs/MISSION.md`, `docs/VISION.md` |
| Roadmap | release direction | `docs/ROADMAP.md` |
| First-use guide | install and first useful local run from crates.io | `docs/FIRST_USE.md` |
| First-hour guide | maintainer first-hour walkthrough from install to one credible review action | `docs/FIRST_HOUR.md` |
| CLI guide | current user-facing commands and artifact surfaces | `docs/CLI.md` |
| CI and PR artifacts | advisory CI lanes, first-pr packets, comment plans, and future trusted poster design | `docs/ci/PR_CI.md`, `docs/ci/COMMENT_PLAN_EXAMPLES.md`, `docs/ci/TRUSTED_COMMENT_POSTER.md` |
| GitHub Actions user guide | copy-paste drop-in workflow for downstream Rust repositories | `docs/ci/github-actions.md`, `.github/examples/unsafe-review-first-pr.yml` |
| Coverage telemetry | advisory cargo-llvm-cov + Codecov execution-surface signal, not unsafe correctness | `docs/ci/coverage.md`, `.github/workflows/coverage.yml` |
| Explanations | concept notes, trust boundaries, reviewer examples, and agent packet examples | `docs/explanation/` |
| Editor workflow | saved read-only editor projection walkthrough | `docs/editor/saved-lsp-json.md` |
| Editor extension MVP | publishable saved-lsp viewer extension scope and non-goals | `docs/editor/extension-mvp.md` |
| Editor marketplace publication | owner pre-publication checklist and manual publish workflow | `docs/editor/marketplace-publication.md`, `.github/workflows/editor-publish.yml` |
| Proposals | why a workstream exists | `docs/proposals/` |
| Releases | release targets, readiness notes, and publication receipts | `docs/releases/` |
| Specs | behavior contracts | `docs/specs/` |
| ADRs | durable architecture decisions | `docs/adr/` |
| Templates | reusable proposal, spec, plan, closeout, and publication receipt skeletons | `docs/templates/` |
| Contribution process | swarm-to-main routing and source PR requirements | `docs/contributing/SWARM_TO_MAIN.md` |
| Spec style | durable source-of-truth ownership boundaries and artifact role split | `docs/spec-style.md` |
| Spec rails guide | contributor workflow for source-of-truth rails and tool-state boundaries | `docs/contributing/spec-rails.md` |
| Implementation plans | PR-sized sequence and proof commands | `plans/` |
| Active lane | current dogfood-calibrated evidence loop | `docs/status/DOGFOOD_CALIBRATED_EVIDENCE_LANE.md` |
| Dogfood index | selected real-crate dogfood targets, usefulness notes, and recorded outcome movement | `docs/dogfood/index.md`, `docs/dogfood/index.json`, `docs/dogfood/usefulness-notes.md` |
| Fuzzing | manual analyzer robustness harness and input contract | `docs/FUZZING.md` |
| Support summary | concise support posture front panel | `docs/status/SUPPORT_SUMMARY.md` |
| Support tiers | detailed product claim to proof ledger | `docs/status/SUPPORT_TIERS.md` |
| Objective audit | current objective evidence and remaining gaps | `docs/status/OBJECTIVE_AUDIT.md` |
| Policies | ledgers, baselines, suppressions | `policy/` |

Rule: do not make every document do every job. Proposals say why, specs say what,
ADRs say why this architecture, plans say how, and policies hold exceptions.
