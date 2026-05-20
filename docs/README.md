# Documentation map

| Layer | Owns | Path |
|---|---|---|
| Mission / vision | product purpose and end state | `docs/MISSION.md`, `docs/VISION.md` |
| Roadmap | release direction | `docs/ROADMAP.md` |
| First-use guide | install and first useful local run from crates.io | `docs/FIRST_USE.md` |
| CLI guide | current user-facing commands and artifact surfaces | `docs/CLI.md` |
| Explanations | concept notes, trust boundaries, and reviewer examples | `docs/explanation/` |
| Proposals | why a workstream exists | `docs/proposals/` |
| Specs | behavior contracts | `docs/specs/` |
| ADRs | durable architecture decisions | `docs/adr/` |
| Implementation plans | PR-sized sequence and proof commands | `plans/` |
| Active lane | current dogfood-calibrated evidence loop | `docs/status/DOGFOOD_CALIBRATED_EVIDENCE_LANE.md` |
| Dogfood index | selected real-crate dogfood targets and recorded outcome movement | `docs/dogfood/index.md`, `docs/dogfood/index.json` |
| Fuzzing | manual analyzer robustness harness and input contract | `docs/FUZZING.md` |
| Support summary | concise support posture front panel | `docs/status/SUPPORT_SUMMARY.md` |
| Support tiers | detailed product claim to proof ledger | `docs/status/SUPPORT_TIERS.md` |
| Objective audit | current objective evidence and remaining gaps | `docs/status/OBJECTIVE_AUDIT.md` |
| Policies | ledgers, baselines, suppressions | `policy/` |

Rule: do not make every document do every job. Proposals say why, specs say what,
ADRs say why this architecture, plans say how, and policies hold exceptions.
