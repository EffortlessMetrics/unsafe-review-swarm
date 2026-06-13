# 2026-06-13 — External-PR validation plan (read-only, post-0.3.7)

Status: PREP. Not yet runnable. **[OWNER] Do not start until `unsafe-review` 0.3.7 is published to crates.io.** As of this writing the latest published/source release is **v0.3.6** (2026-06-11), the workspace crates are at version `0.3.6`, and the composite action pins `version: "0.3.6"` (`.github/actions/unsafe-review-first-pr/action.yml:46`). Running this plan before 0.3.7 publishes would characterize a stale binary that predates the 0.3.7 fixes (e.g. `pub(crate)`/`pub(super)` visibility #1666 [MERGED], spread-aware selection under `--max-cards` #1667 [MERGED], the `RequiresWitnessReceipt`/telemetry-over-count truthfulness fix from #1633 [MERGED], bounded comment body #1646 [MERGED]). **[OWNER] When 0.3.7 publishes, bump the install pin in `action.yml:46` (or pass `--version`) and only then execute.** Publishing and the pin-bump are owner-gated and tracked under #1659 — do not perform them autonomously.

## Trust boundary (fixed — read before writing any finding)

`unsafe-review` is **advisory static unsafe-contract review**. It finds unsafe Rust changes missing a safety contract, guard, test, or witness. This validation pass is a **noise/usefulness characterization, NOT calibrated precision/recall, NOT an accuracy percentage, NOT a benchmark.** Every finding in this pass and every issue it produces must avoid the forbidden vocabulary: memory-safety proof, "finds UB" / UB-free, Miri-clean (absent a matching witness receipt), site / site-execution proof, calibrated precision/recall, "zero false positives", default comment-posting, default blocking, witness execution by the tool, silent source edits. unsafe-review is the evidence layer; ub-review is the orchestrator. This document inherits the same posture as `docs/dogfood/REAL_WORLD_FINDINGS.md` ("The corpus is a usefulness instrument, not a benchmark") and `docs/handoffs/2026-06-07-0.3.4-consumer-validation.md`.

## The read-only rule (hard constraint)

The target is a **third-party / foreign fork** (the Bun UB-hunt fork). This pass is **STRICTLY READ-ONLY** against that target:

- **Check / verify / document / surface only.** Run `first-pr` against a checked-out PR diff and capture artifacts. Nothing else.
- **NEVER alter those PRs** — no comments, no reviews, no pushes, no suggested edits, no `baseline init` against the foreign repo. (`baseline init` writing a snapshot into `--root/policy/` was the side-effect filed as #1543, now CLOSED/COMPLETED; keep the discipline regardless — never run `baseline init` on a foreign working tree.)
- **NEVER file third-party-repo issues autonomously.** No issues, discussions, or comments on the Bun fork or upstream Bun. All findings from this pass are filed as **narrow issues in OUR repo only** (`EffortlessMetrics/unsafe-review-swarm`), building on the existing decision issues #1659 (owner-gated adoption readiness, OPEN) and #1665 (dogfood real-crate gaps, OPEN) — do not file duplicates of those.
- The tool's own posture already guarantees the read-only floor: the composite action "does NOT post comments, does NOT request write tokens, does NOT run witnesses, does NOT edit source files" (`action.yml:6-14`). This plan must not bypass that floor.
- Operate against a **local clone/checkout** of the fork on a private branch; treat the working tree as immutable input. Never push.

## Target selection criteria

Pick **one PR (or a small, two-to-three-file fork delta)** that satisfies all of:

1. **Touches Rust unsafe seams in the diff.** The PR must change Rust code containing (or adjacent to) `unsafe` blocks/fns, raw pointers, FFI boundaries, or `MaybeUninit` — otherwise `first-pr` correctly emits zero cards and there is nothing to characterize. (Recall the 0.3.4 lesson in the consumer-validation handoff: a canonical Bun seam in safe Rust *above* the unsafe-block level produced 0 cards, #1544 — so confirm the diff actually crosses the unsafe-block level.)
2. **Real, recent PR diff** — a genuine fork change, not a synthetic patch, so the noise reading reflects real-world review pressure (the open item flagged in `REAL_WORLD_FINDINGS.md`: "Real external-repo PR noise reading").
3. **Bounded scope.** Prefer a diff small enough to scan in well under the per-file cost ceiling. Bun is a brownfield JS-runtime repo where per-file scan cost is high (documented at ~28–36s for 700–1100-line files; full `src/jsc/**` intractable — #1546). Scope to the changed files via the diff, not a whole-repo scan.
4. **Mixed-language context is acceptable and interesting** — a JS/TS oracle over a Rust/native seam is exactly the Bun shape (`docs/dogfood/ripr-bun-diff-first-requirements.md`), but the seam under review must be Rust for `first-pr` to produce cards.
5. **Has at least one "expected seam"** the reviewer can name in advance, so the pass can test for a *missed expected seam* (see rubric).

Record the exact PR URL, head SHA, base SHA, and the rationale for selection before running.

## Exact commands

All commands are **read-only** and run against a local checkout. Replace `<bun-clone>`, `<pr-head>`, `<pr-base>` with concrete values; pin `--version 0.3.7` once published.

```bash
# 0. Acquire the tool (time this — install/acquisition is a cost metric).
#    Cold cargo-install is ~45s per the owner-gated note in #1659 item 2.
cargo install unsafe-review --locked --version 0.3.7

# 1. Materialize the PR diff against its base (read-only). Capture to a file.
git -C <bun-clone> diff <pr-base>..<pr-head> -- '*.rs' > ./bun-pr.diff

# 2. Primary run: first-pr against the real PR diff, full advisory bundle.
#    first-pr always writes the full bundle to --out-dir; it does NOT accept
#    --policy/--format/--json/--out (parse.rs:482-503). It is advisory: posts
#    nothing, runs no witnesses, edits nothing.
unsafe-review first-pr \
  --root <bun-clone> \
  --diff ./bun-pr.diff \
  --out-dir ./bun-validation/first-pr

#    Capture the process exit code (0 advisory pass incl. findings;
#    1 policy/new-debt; 2 tool error) and wall time.

# 3. Optional context run: bounded repo scan over ONLY the changed subtree,
#    to read repo-scan partial/stop_reason status honesty (commit 589b24c7 /
#    #1562 emits partial status with stop_reason when --max-cards caps a scan;
#    07c15969 / #1561 treats valid empty diffs as no-op). Scope tightly.
unsafe-review repo \
  --root <bun-clone> \
  --include 'src/<changed-subtree>/**' \
  --max-cards 50 \
  --max-files 50 \
  --timeout-seconds 120 \
  --out ./bun-validation/repo-status.json
```

Notes on flags (verified against source): `--root`, `--base`, `--diff`, `--format`, `--policy`, `--json`, `--markdown`, `--out`, `--max-cards` are the **shared** check options (`crates/unsafe-review-cli/src/parse/check_parse.rs:17-83`), so `--out` in step 3 is a valid shared option even though it is not a `repo`-specific flag. `first-pr` adds only `--out-dir` (`parse.rs:509-516`), explicitly rejects `--out`/`--format`/`--policy`/`--json`/`--markdown` (`parse.rs:482-503`), and defaults `--base origin/main` when neither base nor diff is given (`parse.rs:520-522`). `repo` adds `--include`/`--exclude`/`--max-files`/`--timeout-seconds`/`--list-files`/`--progress` on top of the shared options (`parse.rs:545-613`). Do **not** run `baseline init` against the foreign repo (see read-only rule; #1543 is closed but the discipline stands).

## Artifact set to capture

Copy the entire `--out-dir` bundle out to an analysis location (under OUR repo's `target/` or a scratch dir — do **not** commit the foreign repo's scanned source). The composite action documents and verifies the bundle shape (`action.yml:71-177`). Capture all of:

| Artifact | What it answers |
|---|---|
| `review-kit.json` | The full structured optic (the ReviewCard projection root). |
| `unsafe-review-gate.json` | Advisory coverage movement + `status` field (NOT a merge verdict, `action.yml:80-85`); routed by `schema_version` `unsafe-review-gate/v1`. |
| `cards.json` | Per-card detail: operation family, obligation, missing evidence, class, suggested route. The primary noise/usefulness object. |
| `comment-plan.json` | The bounded, budget-capped inline-comment candidates (what a maintainer *would* see); `comment_plan_status` (e.g. `not_eligible` for inherited-only). |
| `repair-queue.json` | Classification + prose guidance, now including an `applicable_edit` field per bucket (#1542, CLOSED/COMPLETED 2026-06-11 — `repair_queue.rs:136,162`). Verify the field is populated on real cards. |
| `receipt-audit.md` (and `.json` if emitted) | Witness-receipt audit surface. |
| `github-summary.md` | The bounded job-summary surface a maintainer reads first. |
| repo-scan status (`repo-status.json` from step 3) | Partial vs complete honesty: `status`, `stop_reason` when `--max-cards`/timeout caps (#1562). |
| `usefulness-telemetry.json` | Self-reported card counts / readiness states (optional in older binaries — verify present & non-empty for 0.3.7; `action.yml:170-177`). |

Also capture (present in the bundle per `action.yml:74-78,145-159`): `pr-summary.md`, `cards.sarif`, `witness-plan.md`, `manual-candidates.json`, `manual-repair-queue.json`, `tokmd-packets.json`, `lsp.json`. Record the **byte size of each** (artifact-size cost metric) and a sha256 of the bundle for reproducibility.

## Classification rubric

Classify each emitted card (and each *expected-but-absent* seam) into exactly one of these. This mirrors `docs/dogfood/triage-taxonomy.md` and the judgment vocabulary in `REAL_WORLD_FINDINGS.md`; it is a usefulness label, not a correctness verdict.

- **actionable** — card carries a specific next action (a guard to add, a contract to write, a witness route to follow) a maintainer could act on.
- **inherited** — pre-existing unsafe gap surfaced but not introduced by this diff (`class = inherited`, `new_gaps=0`); should be visible in the kit but **quiet** in the comment plan (`comment_plan_status = not_eligible`).
- **not-selected-correctly** — wrong-target citation: the card fires on a site the diff does not actually change, or attributes the wrong operation family (the 0.3.4 / arrayvec correction-1 pattern).
- **noisy** — technically present but does not improve reviewability for its stated purpose; or duplicate/structurally-identical card spam (e.g. the SIMD `#[target_feature]` explosion, #1665 item 4).
- **missed expected seam** — a Rust unsafe site in the diff the reviewer expected a card for, but none fired (distinguish cap-driven omission from a genuine classifier gap; cf. spread-aware selection #1667 / #1665 item 1 and the ByteStream safe-Rust gap #1544).
- **agent-ready** — card + route are concrete enough that an automation lane (ub-review) could act without fabricating an edit; check this is *consistent* across surfaces (the #1633 fix was a truthfulness correction where readiness surfaces disagreed / telemetry over-counted).
- **human-review-only** — card correctly identifies a hard boundary (FFI layout, concurrency interleaving) routed to human review / a witness receipt, not auto-repair.
- **docs confusion** — output wording the reviewer found misleading or that risked overclaiming the trust boundary.
- **cost friction** — the run cost (time/size) materially impeded the review (e.g. per-file scan cost #1546).
- **artifact friction** — a consumer (maintainer or agent) could not use an artifact as shipped. Note that the two prior friction items here landed before 0.3.7: `applicable_edit` (#1542) and the `--max-cards` cap status artifact (#1545, parity with `--timeout-seconds`, plus #1562) are both CLOSED/COMPLETED — **confirm they are actually present/populated in this run** rather than re-filing them.

Also note, per card, the **class movement** vocabulary where relevant: `contract_missing` → `guarded_unwitnessed` is *improvement* (reclassification), not *resolution*; resolution only occurs when a site leaves diff scope (`REAL_WORLD_FINDINGS.md`: "Evidence improvement registers as reclassification").

## Cost metrics

Record concretely for the run:

- **Wall time** of `first-pr` (and the optional bounded `repo` scan) end to end.
- **Install / acquisition time** (cold `cargo install`, step 0 — the ~45s cold-start the prebuilt-binary item in #1659 would remove).
- **Artifact size** — per-file and total bundle bytes.
- **Files scanned** — count of Rust files the diff touched / the scan visited.
- **Cards emitted** — total, and per operation family.
- Plus any `stop_reason` / partial status (truncated vs complete), so cost numbers are not silently from a capped run.

## Success questions

Answer each with the captured evidence cited:

1. **Would a maintainer tolerate this?** Is the volume and framing of cards/comments low-noise enough to leave on for a real PR, given the brownfield reality?
2. **Would the selected comments help?** Do the `comment-plan.json` candidates name a real gap + a concrete next action + the trust boundary, anchored to the right `path:line`?
3. **Would the omitted ones stay quiet for the right reasons?** Are inherited/out-of-scope gaps correctly *not* in the comment plan (`not_eligible`), rather than silently dropped because of a cap?
4. **Would an agent know what to do?** Are `agent-ready` cards consistent across surfaces and concrete enough for ub-review to act without fabricating an edit?

## Boundary on outputs / disposition

This is a characterization pass, not a calibration. **State the boundary in every finding.** File results as **narrow, evidence-backed issues in `EffortlessMetrics/unsafe-review-swarm` only**, each citing the exact artifact + path:line that exposed it, and **build on #1659 and #1665** rather than duplicating them (e.g. if the run re-confirms cap behavior or the #1546 cost ceiling, add evidence to or cross-reference the existing issue). Before filing anything new, re-verify against this 0.3.7 binary that the already-CLOSED items stayed fixed — #1542 (applicable edits), #1545 (`--max-cards` cap status), #1543 (baseline-init side-effect) — and only file a regression if the evidence shows one. No third-party issues, no PR mutation, no witness execution, no source edits.
