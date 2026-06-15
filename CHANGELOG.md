# Changelog

This changelog starts with the post-0.3.2 unsafe-review workbench usability
lane. Earlier release targets and publication notes live in
[`docs/releases/`](docs/releases/).

`unsafe-review` remains advisory static review evidence. It does not prove UB,
memory safety, UB-free status, Miri-clean status, site execution, calibrated
precision/recall, or policy readiness, and it does not run witnesses, post
comments, edit source, or block by default.

## Unreleased

### Changed

- `policy report` `baseline_state` column now projects the canonical 5-value
  coverage-movement vocabulary (`new`, `worsened`, `inherited`, `resolved`,
  `unknown`) from `CoverageBlock::derive` with snapshot-slot movement applied —
  the same value `cards.json` and the agent packet project (SPEC-0030
  §single-truth). Policy classification (`new_gap`, `baseline_known`,
  `suppressed`, `non_actionable`) is carried by the `policy_status` and
  `policy_reason` fields, which are unchanged. Advisory only.

## 0.3.7 - 2026-06-14

0.3.7 — adoption, telemetry, and real-code low-noise improvements. It makes
`unsafe-review` easy to adopt and measured in use, tightens how findings are
framed and ranked on real code, and delivers a batch of evidence-discipline
correctness fixes across all output surfaces. It adds no analyzer breadth and
no new claim: it remains advisory static unsafe-coverage evidence, not a proof,
not a policy gate, and not a substitute for code review.

### Added

- Composite **GitHub Action** (`unsafe-review-first-pr`) for two-line CI
  adoption: wraps `first-pr`, uploads the review kit, appends the step summary,
  and exposes the gate manifest. Advisory by default — no automatic
  comment-posting, no blocking, no inherited-fail. Includes `fetch_depth`
  input, version-skew hardening, and live-smoke validation.
  ([#1628](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1628),
  [#1660](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1660),
  [#1661](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1661),
  [#1662](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1662),
  [#1663](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1663),
  [#1664](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1664))
- **`unsafe-review pr`** zero-config subcommand with repo/base autodetect:
  runs `first-pr` without requiring explicit repo or diff path flags; no
  silent full-repo scan, no ambiguous default blocking.
  ([#1629](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1629))
- Adoption front-door **`docs/START-HERE.md`** and outward-facing docs:
  real-world dogfood narrative and agent-integration guide.
  ([#1635](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1635),
  [#1669](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1669))
- **Scheduled full-corpus backstop + external RSS harness** (SPEC-0039):
  resource measurement lives on the scheduled/bench path, not in-tool;
  ADR-0008 amended to ratify external-first peak-RSS posture.
  ([#1627](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1627),
  [#1631](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1631))
- **Usefulness telemetry** projected from `ReviewCard`: cards/PR,
  new/worsened/resolved/inherited, selected vs not-selected reasons,
  agent-ready vs human-only, `scan_cost` (elapsed_ms/output_bytes),
  not-selected-class histogram, unfulfilled-obligation count. Telemetry
  is a usage/subset signal — not calibrated precision/recall.
  ([#1630](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1630),
  [#1634](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1634),
  [#1647](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1647))
- `unsafe-review-gate.json` emitted from `repo` mode (SPEC-0034 parity).
  ([#1705](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1705))
- Per-card **coverage-movement projection** from the summary (worsened,
  resolved, inherited, unchanged counts per card).
  ([#1716](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1716))
- `unsafe-review+` badge is now **baseline-movement-aware**: reflects
  resolved/worsened delta, not just card count.
  ([#1717](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1717))

### Changed

- **SARIF level and LSP severity derived from card class** (single-source
  truth): no more LSP severity-1 for low-class cards; SARIF rule description
  is class-aware instead of always "missing safety contract."
  ([#1706](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1706),
  [#1713](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1713))
- **Owner cards grouped in human surfaces** (PR summary, human CLI) while kept
  in artifacts, counts, and SARIF: reduces surface noise without suppressing
  evidence.
  ([#1710](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1710))
- **`agent_lsp_readiness` and agent-packet readiness** derived from card class
  and `RequiresWitnessReceipt`: all output surfaces now agree on agent
  readiness; coverage no longer claims "agent-ready" while the comment plan
  says `requires_witness_receipt`.
  ([#1633](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1633),
  [#1632](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1632))
- **Comment-plan importance ranking**: top-N selected by priority → gap-severity
  → confidence → file/line (not file order); bounded comment body (220 words,
  single-sourced constant).
  ([#1645](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1645),
  [#1646](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1646))
- **Receipt formal-tool provenance**: `site_reached` strength now requires
  tool provenance in the receipt; receipt records tool name and input hash.
  ([#1707](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1707))
- **Outcome markdown** includes unchanged cards to match JSON parity.
  ([#1708](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1708))
- `summary.unsafe_sites` is now the real pre-cap site count, not a
  post-selection estimate.
  ([#1718](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1718))
- Policy-report movement counts projected from the canonical summary.
- `comment_plan_status` populated from comment-plan selection results.
- GitHub-summary top-card ranked to match PR-summary (single-truth surface).
- Exit-code mapping: `cargo-unsafe-review` exits 1 on policy violation,
  2 on tool error.
  ([#1559](https://github.com/EffortlessMetrics/unsafe-review/issues/1559))
- `cli`: distinguish capped-success from timeout in repo-scan status output.

### Fixed

- **`debug_assert` does not discharge runtime guard obligations**: a
  `debug_assert!` is not a runtime guard — it is stripped in release builds.
  ([#1715](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1715))
- **Test reach requires call-shaped owner use**: bare name mentions in
  non-call positions (type annotations, comments) no longer count as test
  ownership; self-reach rejected.
  ([#1709](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1709))
- **`pub(crate)`/`pub(super)` visibility** miscategorization fixed: these
  are not public-API unsafe exposure.
  ([#1666](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1666))
- **Spread-aware card selection**: a `--max-cards` cap no longer blinds whole
  subsystems by concentrating budget on one file.
  ([#1667](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1667))
- Bare-name unsafe-op detectors gated on `unsafe { }` scope (F1 root fix):
  detectors no longer fire on safe-context or definition-only occurrences.
- `&mut *expr` / `&*expr` classified as `raw_pointer_deref`, not
  `unsafe_pointer_cast`.
- `fn zeroed()` definition no longer carded as a zeroed call.
- Raw-pointer alignment guard anchored to receiver/position (not any guard
  in scope); same-receiver null-check required for pointer-load discharge.
- `set_len` discharge requires joined `new_len <= recv.len()` predicate.
- Atomic-pointer-state tokens bound to the same call, not line-proximity.
- Reach-scan owner matched on identifier boundaries (not raw substring).
- Combined-diff `@@@` header `+` range seeds `new_line` correctly.
- Input: combined-diff `@@@` headers now seed new-line numbers from the
  `+` range correctly.
- FFI unqualified-name boundary: method-receiver dot no longer triggers an
  FFI detection.
- Resolved/inherited movement scoped to diff-touched files only.
- Syntax site wins for multiline `ptr::copy` and plain `transmute`.

### Framing and evidence discipline

The stance/projection program (#1705–#1718) corrects systematic output
framing errors found during real-code validation. These are not new
detections: they are consistency fixes so every surface — JSON, SARIF, LSP,
PR summary, outcome markdown, badges, comment plans, agent packets — projects
from the same `ReviewCard` truth object with evidence-disciplined framing.
No proof, UB-free, Miri-clean, or calibrated-precision claim is added.
Dogfood validation of these fixes (#1700, #1720) showed measurable noise
reduction on 5 fresh real crates.

## 0.3.6 - 2026-06-11

0.3.6 is the CLI-truthfulness and evidence-intake patch. It makes the command
line hard to misuse and lets real external evidence enter the receipt ledger:
unknown flags now fail loudly instead of being silently ignored, a `repo --out`
run that is killed before it finishes still leaves a status artifact, and
runtime sanitizer witnesses can be imported as evidence. It adds no analyzer
breadth. It remains advisory static coverage evidence: it does not prove memory
safety, UB-free status, Miri-clean status, site execution, calibrated
precision/recall, or policy readiness, and it does not run witnesses, post
comments, edit source, or block by default.

### Added

- `receipt import-sanitizer --allow-runtime` accepts runtime/program
  AddressSanitizer (and similar) witness logs as receipt evidence without
  requiring a `test result: ok` line. A run where the sanitizer fires records
  verdict `confirmed` (a failure was observed — not a safety claim); a clean
  run records `not_reproduced` (no signal observed this run — not a safety
  claim). The accepted `--strength` values
  (`configured`/`ran`/`test_targeted`/`site_reached`/`reviewed`) are now listed
  in CLI help and in the unknown-value error.
  ([#519](https://github.com/EffortlessMetrics/unsafe-review/issues/519))
- Auto-emitted cards for the stable-byte-source operation families now carry an
  additive `stable_byte_sub_class` hint (omitted for other families). It is a
  heuristic aperture label, not a memory-safety, UB-free, Miri-clean, or
  site-execution claim.
  ([#1571](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1571))
- Repair-queue entries now carry a bounded `applicable_edit` hint for
  repairable buckets so consumers can render one-click suggestions; entries in
  the terminal `do_not_auto_repair` / `requires_human_review` buckets carry no
  such hint. unsafe-review still does not edit source.
  ([#1542](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1542))

### Changed

- Unknown CLI flags now fail loudly instead of being silently ignored. In
  particular `first-pr --out ...` previously exited 0 and wrote a review kit to
  the default location while ignoring the flag; it now exits 2 with a diagnostic
  that names the flag and suggests `--out-dir`, and writes no bundle.
  ([#531](https://github.com/EffortlessMetrics/unsafe-review/issues/531))
- `confirm` now surfaces command provenance (analyzer-derived route vs
  `--command` override) in dry-run and pre-execution output, and validates
  owner-derived argv tokens so a flag-shaped or shell-ish owner can never be
  spliced into a routed witness command; an invalid owner yields no command
  rather than a fabricated one. `runtime_executed` is never fabricated.
  ([#1514](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1514))
- `unsafe-review repo --out <path>` now writes a "scan started" status stub to
  `<path>.status.json` before analysis begins, so a run killed before it can
  finish still leaves a durable, honest record that a scan was attempted
  (`phase: "discovering"`, `completed: false`). Every normal, capped, timed-out,
  or interrupted outcome supersedes the stub.
  ([#518](https://github.com/EffortlessMetrics/unsafe-review/issues/518))

## 0.3.5 - 2026-06-08

0.3.5 is the instrument-truthfulness patch. It makes `unsafe-review`'s core
assertions exact for downstream automation: what scope ran, whether the diff
input parsed, which exit category occurred, what receipt applies, whether a
scan completed or was capped, and how to scope large repositories. It adds no
analyzer breadth. It remains advisory static coverage evidence: it does not
prove memory safety, UB-free status, Miri-clean status, site execution,
calibrated precision/recall, or policy readiness, and it does not run
witnesses, post comments, edit source, or block by default.

### Added

- CLI guide now documents per-file scan cost and the large-repo/brownfield
  scoping workflow (narrow with `--include`/`--exclude`, preview with
  `--list-files`, bound with `--timeout-seconds`/`--max-cards`, re-run on a
  narrower scope), so whole-repo scans of large repositories are scoped
  deliberately rather than timing out.
  ([#1546](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1546))

- `repo --max-cards N` capped scans now emit a partial status sidecar with
  `partial: true`, `stop_reason: "max_cards"`, `cap: N`, `cards_found: N`, and
  a cap-specific `operator.next_action` ("narrow include/exclude filters or raise
  --max-cards…").  A capped scan exits 0 — it is a successful-but-bounded run, not
  a failure.  Previously a capped scan emitted an unconditional `phase: "complete"`
  + `completed: true` sidecar, making truncated scans indistinguishable from
  complete ones.  All stop reasons now emit a `stop_reason` field: `"none"` for
  a full complete scan, `"max_cards"` for a cap-stopped scan, `"timeout"` for a
  timed-out scan, `"terminated"` for a signal-interrupted scan, and `"error"` for
  an analysis or report-write failure.  Timeout and error share the
  `phase: "failed"` status but are distinguished by `stop_reason`, so a disk-write
  or internal error is never mislabeled as a timeout.  A `partial` boolean
  accompanies the field (`false` only for `stop_reason: "none"`).
  ([#1545](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1545))

- SPEC-0035 (`repo-scan-status/v1` diagnosability) field names corrected to match
  the shipped JSON sidecar: `scan_scope` (not `scope`), `elapsed_ms` (not
  `elapsed_seconds`), `files_discovered`/`files_scanned`/`files_remaining` (not
  `discovered`/`scanned`/`remaining`), `cards_found` (not `cards`), `completed`,
  `partial`, `stop_reason`, `cap`, `error`, `signal`, `partial_path`, `operator`.
  Phase vocabulary corrected to match shipped code: `discovering | scanning |
  complete | failed | terminated` (the spec previously declared `rendering | done |
  timed_out` which were never implemented).  Timeout stays `phase: "failed"` +
  `stop_reason: "timeout"` — no breaking rename.
  ([#1545](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1545))

- `cards.json` and `check`/`repo` `--format json` output bump `schema_version`
  from `0.1` to `0.2` and now carry provenance metadata: top-level
  `tool_version`, plus a `provenance` block with `tool_version`,
  `generated_at` (RFC3339 UTC), resolved absolute root (`root_abs`), resolved
  `base_sha`/`head_sha` in `--base` mode, `diff_path` + `diff_sha256` (SHA-256
  content digest) in `--diff <file>` mode, and a `dirty_worktree` marker.
  Fields that cannot be resolved are omitted, not null. The bump is additive —
  every existing field is unchanged — so consumers routing on `schema_version`
  should accept `0.2` wherever they accepted `0.1`. Partial/interim repo
  reports still emit `0.1` without provenance pending the SPEC-0035
  partial-status reconciliation.
  ([#1517](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1517))

### Changed

- Repo discovery now skips any subdirectory containing a `.git` entry — a nested
  git checkout (`.git` directory) or a gitfile worktree (`.git` file) — by
  default, so scratch worktrees and vendored repository copies no longer inflate
  scan counts as if they were the target repository. The skip is independent of
  gitignore handling. All scan surfaces (repo posture, badges, baseline) share
  the same discovery, so the exclusion applies uniformly.
  ([#1552](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1552))

- A supplied `--diff` whose content does not parse as a unified diff is now an
  explicit input error (exit 2, no analysis, no artifacts) instead of silently
  falling back to a whole-repo scan that still reported `scope: "diff"`. Empty
  or whitespace-only diff input and binary-only diffs remain accepted.
  ([#1516](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1516))

- A supplied `--diff` or `--base` that resolves to an empty diff (e.g. `git diff`
  on a clean branch or an empty diff file) now produces a complete diff-scoped
  no-op run: scope stays `diff`, 0 selected files, 0 cards, and
  `--policy no-new-debt` exits 0. Previously, an empty diff caused the pipeline
  to silently fall back to scanning the whole repository while still reporting
  `scope: "diff"` in artifacts — inflating counts and misrepresenting the scope.
  Callers with clean PRs that relied on the whole-repo fallback should switch to
  `unsafe-review repo` for explicit full-repo analysis.
  ([#1558](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1558))

### Changed (breaking for callers that check exit codes)

- `--policy no-new-debt` violations now exit **1** instead of **2**. The stable
  contract is: 0 = ran to completion (clean or advisory findings); 1 = ran to
  completion, policy found new or worsened coverage gaps; 2 = tool did not
  complete a review (usage, input/IO, or internal error). Callers that tested
  `$? -ne 0` are unaffected; callers that tested `$? -eq 2` to detect policy
  failures should update to `$? -eq 1`.
  ([#1518](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1518))

## 0.3.4 - 2026-06-07

0.3.4 is the coverage-instrument usability patch. It ships the post-0.3.3
coverage-slot model, baseline movement tracking, diff-scoped no-new-debt,
baseline-aware badges, comment-plan gap-anchoring, LSP file:range context,
the `unsafe-review-gate.json` routing manifest, repo-scan diagnosability,
candidate authoring UX, and stable-byte coverage v1. It remains advisory
static coverage evidence: it does not prove memory safety, UB-free status,
Miri-clean status, site execution, calibrated precision/recall, or policy
readiness, and it does not run witnesses, post comments, edit source, or
block by default.

### Added

- Added the SPEC-0029 unsafe-evidence coverage block: a slot-based model that
  assigns each `unsafe` site a coverage slot and tracks which slots have
  evidence, enabling baseline-aware gap reporting.
  [#1529](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1529)
- Added the SPEC-0030 baseline coverage-movement keystone: baseline recording,
  movement tracking, diff-scoped `no-new-debt` reporting, and `worsened_gaps`
  emission. Baseline `init` and `add` authoring subcommands create and extend
  baselines without changing tool advisory posture.
  [#1531](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1531)
  [#1536](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1536)
- Added the SPEC-0031 baseline-aware badge: the coverage badge reflects
  baseline movement state so repos can surface coverage trends without
  implied proof.
  [#1532](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1532)
- Added the SPEC-0032 comment-plan coverage-gap hardening: comment plans are
  now anchored to coverage gaps, preventing phantom anchors and improving
  comment-plan signal quality.
  [#1535](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1535)
- Added the SPEC-0033 file:range context scan for LLM/agent consumers: the
  `context` subcommand now emits a `file:range` context packet with precise
  source span information for LLM and agent consumers.
  [#1534](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1534)
- Added the SPEC-0034 `unsafe-review-gate.json` routing manifest: every
  `first-pr` run writes a structured gate manifest that downstream CI and
  agent consumers can read to route decisions without parsing human output.
  [#1533](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1533)
- Added the SPEC-0035 repo-scan diagnosability: `repo` output now includes
  scan-scope metadata and diagnosability fields so consumers can distinguish
  scanned-but-empty from not-scanned.
  [#1528](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1528)
- Added stale JS-buffer span detection: the analyzer flags use of a stale
  JS `ArrayBuffer`/`TypedArray` span after a GC-reentry point.
  [#1508](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1508)
  [#1538](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1538)
- Added stable-byte coverage v1 with span-as-arg detection and snapshot
  suppression: the scanner detects span values passed as arguments after
  reentry, and suppresses cards when a snapshot is taken before use.
  [#1538](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1538)
- Added candidate `new` and `lint` authoring subcommands for structured
  candidate creation and linting without changing the manual/advisory boundary.
  [#1526](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1526)
- Added opt-in `confirm --allow-heavy` cue executor for running emitted
  confirmation cues with explicit opt-in; `runtime_executed` is projected
  into output. Execution remains opt-in and does not change default advisory
  posture.
  [#1510](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1510)
  [#1509](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1509)
- Added confirmation state projection and cheapest-confirmation ranking to
  output surfaces.
  [#1525](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1525)
- Added dogfood usefulness rollup with a drift rail to catch usefulness
  regressions across real-crate dogfood samples.
  [#1527](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1527)
- Required node-parity oracle maps for Bun-oriented candidates.
  [#1497](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1497)

### Changed

- CI now falls back to full GitHub-hosted gate by default with a single tight
  deterministic core gate plus an advisory ub-review LLM layer.
  [#1507](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1507)
  [#1524](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1524)

### Documented

- Encoded the receipted economic thesis and ownership split in the interop
  north-star.
  [#1530](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1530)
- Added ease-of-use lane specs: SPEC-0028 delivery surfaces and ease of use,
  SPEC-0029 coverage model, SPEC-0030 baseline movement.
  [#1521](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1521)
- Mirrored source 0.3.3 publication into swarm workbench.
  [#1506](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1506)

## 0.3.3 - 2026-06-05

0.3.3 is the Bun manual-candidate cockpit usability patch. It ships the
post-0.3.2 usability lane below. It remains advisory: manual candidates are
manually discovered (`analyzer_discovered = false`), confirmation cues are
emitted but never executed, and no proof, UB-free, Miri-clean, or
site-execution claim is made.

### Added

- Added per-card confirmation cues that frame each finding as a hypothesis
  pending external confirmation: `hypothesis_to_confirm`, `build_this_first`,
  `minimal_repro`, and `confirmation_step` are projected into `cards.json`,
  comment plans, agent context packets, and terminal `first-pr` output, and
  each cue states that unsafe-review did not run it.
  [#1431](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1431)
  [#1433](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1433)
  [#1435](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1435)
  [#1436](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1436)
  [#1456](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1456)
  [#1459](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1459)
- Added stable-byte manual-candidate metadata (`class`, `source`, `sink`,
  `hazard`, `observable`, `proof_required`, `suggested_fix_boundary`,
  `pr_aperture`, `ledger_state`) and surfaced it through `first-pr` and the
  GitHub summary while preserving the manual/advisory boundary.
  [#1422](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1422)
  [#1423](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1423)
- Added manual-candidate oracle maps (`rust_seam`, `oracle_language`,
  `oracle_path`, `oracle_kind`) with required node-parity oracle coverage for
  Bun-oriented candidates; oracle maps are routing context, not witness
  execution.
  [#1441](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1441)
  [#1497](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1497)
- Added ReviewCard proof-path projection across JSON, Markdown, comment-plan,
  witness-plan, and outcome outputs.
  [#1395](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1395)
- Added card evidence projection into `witness-plan.md`.
  [#1404](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1404)
- Added the `tokmd-packets.json` first-pr artifact: formatting-only manual
  packet inputs with comment budget, preset inputs, and manual repair item
  projection, recorded with `tokmd_run = false`.
  [#1412](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1412)
  [#1440](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1440)
  [#1450](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1450)
  [#1452](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1452)
- Added the manual repair handoff path: manual repair sidecar buckets, a
  review-kit manual-candidate mix summary, a repair-queue cockpit panel with
  agent-readiness cues, and repair-queue bucket reasons in summaries.
  [#1425](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1425)
  [#1427](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1427)
  [#1443](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1443)
  [#1449](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1449)
  [#1485](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1485)
- Added diff-scope file counts (`changed_files`, `changed_rust_files`,
  `changed_non_rust_files`) to summary JSON, reviewer summaries, and the
  review-kit manifest.
  [#1355](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1355)
  [#1356](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1356)
  [#1357](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1357)
- Added JSON and Markdown output formats for `repo --list-files` with recorded
  scan scope.
  [#1363](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1363)
- Added a dogfood drift guard requiring the Bun manual-candidate smoke report
  to list every committed manual-candidate example ID and primary file:line.
  [#1501](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1501)
- Added the public find/fix workflow for UB-risk review seams:
  `doctor`, `first-pr`, `pr-summary.md`, `explain`, `context --json`,
  `witness-plan.md`, receipt audit, and outcome comparison now have a single
  maintainer path. See [docs/FIND_AND_FIX_UB.md](docs/FIND_AND_FIX_UB.md).
  [#1337](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1337)
- Added ReviewCard fix recipes by operation family for `get_unchecked`,
  `MaybeUninit::assume_init*`, `Vec::set_len`, UTF-8 unchecked conversion,
  pointer copies, `NonNull`, raw pointer reads/writes, transmute, FFI/unsafe
  calls, and target-feature/inline-asm review. The recipes describe what
  evidence matters, good and bad repairs, witness routes, and what the recipe
  does not prove.
  [#1340](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1340)
- Added the bounded agent repair workflow for `repair-queue.json` and
  `context <card-id> --json`, including allowed repairs, do-not-do rules, stop
  conditions, receipt handling, and reviewer responsibility.
  [#1342](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1342)
- Added the advisory UB-risk CI cookbook: run `first-pr`, upload the review kit,
  append `github-summary.md`, optionally emit SARIF, and avoid automatic
  comments or blocking by default.
  [#1343](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1343)
- Added dogfood usefulness judgment records so real review-kit cards can be
  labeled `actionable`, `noise`, `missed`, `uncertain`, `good-agent-task`, or
  `bad-agent-task` without implying calibrated precision/recall.
  [#1324](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1324)

### Changed

- Unified the ReviewCard trust boundary across output surfaces and aligned
  public review claims: static unsafe contract review only; not memory-safety
  proof, not UB-free status, not Miri-clean status, and not a site-execution
  claim unless a matching witness receipt says so.
  [#1424](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1424)
  [#1491](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1491)
- Projected manual oracle and proof-mode context into the GitHub summary
  manual-candidate guidance.
  [#1490](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1490)
- Made repair-queue agent readiness a closed contract:
  `ready_for_agent`, `requires_human_review`, `requires_witness_receipt`, and
  `unsupported`. The verifier now enforces that `ready = true` means
  `ready_for_agent`, and `ready = false` means any non-agent-ready state.
  [#1332](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1332)
- Added PR disposition policy: out-of-lane aligned work should be deferred,
  drafted, or blocked rather than closed; close only duplicate, superseded,
  rejected, abandoned, or unrecoverable work.
  [#1329](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1329)
- Projected manual candidate evidence and implementer handoff details into the
  candidate list path while preserving the manual/advisory boundary.
  [#1345](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1345)

### Documented

- Recorded stale-span-after-reentry detection and optional confirmation-cue
  execution as known next analyzer work, not implemented behavior.
  [#1503](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1503)
- Verified and documented the public UB-risk review workflow end to end.
  [#1476](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1476)
- Documented the evidence-machine repo style with CI and PR guidance.
  [#1461](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1461)
- Closed out the `get_unchecked` applicability burst with a maintained handoff
  covering pinned controls, false-positive rails, unclaimed limits,
  fixture-only versus dogfood-observed status, and fix-recipe mapping.
  [#1334](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1334)
- Promoted the usability docs and repair-queue readiness contract to the public
  source repository with history-preserving source catch-up.
  [source #520](https://github.com/EffortlessMetrics/unsafe-review/pull/520)
