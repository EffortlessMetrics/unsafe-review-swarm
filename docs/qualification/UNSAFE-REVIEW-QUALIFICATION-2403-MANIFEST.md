# Installed-candidate qualification row manifest — issue #2403 (harness prep)

Controlling issue:
[https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2403](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2403).
Work spec:
`plans/work-specs/UNSAFE-REVIEW-WORK-2403-HP1.toml`.

This slice prepares the harness only. It performs no installed execution, renders
no qualification verdict, and touches no network. Every execution row below is
`blocked` until the frozen #2401 matrix names the candidate and source #579
records the package identities.

Format choice: Markdown table, matching the human-readable matrix style of
`docs/status/INSTALLED_QUALIFICATION.md`. Machine-readable qualification
contracts stay in `plans/release-cutline/`; this manifest is the prep-lane
working map, not a second contract.

## Row map

| Row | Acceptance area | Fixture directory | Harness entry point | Verdict rule | Status / owner |
| --- | --- | --- | --- | --- | --- |
| `installed-cli-smoke` | Installed CLI identity, help, failure semantics | `fixtures/raw_pointer_deref_brownfield_inherited` (failure rows); identity rows need no fixture | `.github/workflows/windows-qual.yml` ("Run qualification rows") with `.github/scripts/installed_consumer_probe.py --binary <install-prefix>/bin/unsafe-review` | `pass` iff `--version` matches the candidate receipt, every help row exits 0, invalid-flag and malformed-diff rows exit 2, and no fallback binary is used; anything else is `fail` | `blocked` — frozen matrix owner (issue #2401); source identities owner (source #579) |
| `installed-lsp-agent-loop` | LSP/agent loop with saved diagnostics | `fixtures/raw_pointer_alignment` plus the scripted #1887 client fixture revision | `xtask/src/lsp_smoke.rs` protocol (diagnostic, hover, code action, agent packet, witness route, witness command), bound to the installed binary at execution; saved bundle `target/unsafe-review/lsp.json` | `pass` iff every loop step agrees on one card identity and the saved bundle validates against schema `0.2`; anything else is `fail` | `blocked` — frozen matrix owner (issue #2401); source identities owner (source #579) |
| `consumer-tokmd-ingestion` | Named-consumer ingestion: tokmd presets | Producer bundle `tokmd-packets.json` (`tokmd-packets/v1`, `tokmd_run=false`); pins: tokmd `1.15.0` at `3d278c56d4afe37583e67500fc2e89e60c3077fe` (five presets, see `docs/handoffs/2026-08-08-tokmd-packets-1857-acceptance.md`) | The five-preset render commands recorded in the #1857 acceptance handoff, run against the candidate bundle | `pass` iff all five presets render the candidate bundle; anything else is `fail` | `blocked` — frozen matrix owner (issue #2401); source identities owner (source #579) |
| `consumer-ub-review-ingestion` | Named-consumer ingestion: ub-review parser | Candidate bundle gate (`unsafe-review-gate/v1`) plus comment-plan (`0.1`); pins: ub-review `0.1.2` at `55e4fbab879b3a83c31f35cdc9f7e5cd99f0a6c8`, parser `src/sensors/unsafe_review.rs::read_unsafe_review_artifacts` | The verification procedure in `docs/receipts/ub-review-smoke-6f595070.md` (gate plus envelope shape check against the real parser contract) | `pass` iff the gate routes, the `0.1` envelope ingests, and card identity, operation family, and trust boundary survive; anything else is `fail` | `blocked` — frozen matrix owner (issue #2401); source identities owner (source #579) |
| `consumer-saved-editor` | Named-consumer ingestion: saved editor bundle | Saved bundle `target/unsafe-review/lsp.json` (schema `0.2`); viewer `editors/vscode` (saved-bundle MVP); procedure `docs/editor/saved-lsp-json.md` and `docs/editor/extension-mvp.md` | Load the candidate saved bundle in the pinned viewer exactly as `docs/editor/extension-mvp.md` describes | `pass` iff the saved bundle loads with diagnostics, hovers, and command data intact; no live-server availability is claimed; anything else is `fail` | `blocked` — frozen matrix owner (issue #2401); source identities owner (source #579) |
| `space-in-install-prefix` | Install and execution paths containing spaces | Task-owned install prefix containing spaces (created at execution; no committed fixture) | Install with `cargo install --root "<install prefix with spaces>"`, then invoke `"<install prefix with spaces>/bin/unsafe-review" --version`; quoting mirrors `init_shell_arg` in `.github/scripts/installed_consumer_probe.py` | `pass` iff the install and invocation work with the exact path quoted; otherwise a loud failure quoting the exact path — never a silent fallback | `blocked` (execution) — frozen matrix owner (issue #2401); quoting logic proven by unit tests in this slice |
| `harness-selection-tests` | Installed-LSP executable selection negatives and fixture-pin recording | None (unit tests against the harness selection logic and recorded pins) | `xtask/src/qualification_manifest.rs` (`cargo test --locked -p xtask qualification_manifest`) | `pass` iff a wrong or missing executable fails loudly naming the expected path with zero fallback attempts, space paths quote exactly, and every pin above resolves to a recorded revision and an existing repo path | `ready` — proven in this slice (issue #2403 HP1) |

## Verdict vocabulary

`pass`, `fail`, `skipped`, `not_applicable`, or `blocked`, as required by the
`plans/release-cutline/UNSAFE-REVIEW-QUALIFICATION-1921.toml` receipt rule. A
wrong or missing executable is always a loud failure naming the expected path;
silent fallback to `cargo run`, `PATH` lookup, dev binaries, or the installed
release is never an acceptable outcome. Skipped rows must state why.

## Recorded consumer pins (not invented)

| Consumer | Pin | Recorded in |
| --- | --- | --- |
| tokmd | `1.15.0` at `3d278c56d4afe37583e67500fc2e89e60c3077fe`, five presets (`tokmd-packets/v1`) | `docs/status/RELEASE_COMPATIBILITY.md`, `docs/handoffs/2026-08-08-tokmd-packets-1857-acceptance.md` |
| ub-review | `0.1.2` at `55e4fbab879b3a83c31f35cdc9f7e5cd9f0a6c8`, parser `src/sensors/unsafe_review.rs::read_unsafe_review_artifacts` | `docs/receipts/ub-review-smoke-6f595070.md`, `.github/workflows/ub-review.yml` |
| saved editor | Saved LSP schema `0.2` (first-PR verifier rejects `0.1`); viewer `editors/vscode` | `docs/status/RELEASE_COMPATIBILITY.md`, `docs/editor/saved-lsp-json.md`, `docs/editor/extension-mvp.md` |

## Claim boundary

This manifest maps rows to fixtures, harness entry points, and verdict rules. It
proves no installed execution, no qualification verdict, no safety, UB-free,
Miri-clean, site-execution, precision/recall, or release qualification claim.
The frozen #2401 matrix and source #579 identities stay owner-gated; no row
executes until both land.
