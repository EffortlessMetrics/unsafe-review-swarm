> **[OWNER-GATED — PREP ONLY]** The controller (an autonomous Claude session) wrote this runbook from read-only research against live repo state on 2026-06-13. It has **not** published anything, mutated the source repo, created a branch, or run any `cargo` build. Every step that mutates the source repo, publishes a crate, pushes a tag, or cuts a GitHub Release is marked **[OWNER]** and is the owner's to execute. The controller's role ends at this document. Nothing in this session reaches external users until the owner executes the publish steps below: published latest is **0.3.6** (crates.io, 2026-06-11), and 0.3.7 ships the entire post-0.3.6 adoption/telemetry lane.

# 0.3.7 release readiness — adoption, telemetry, and real-code low-noise improvements

## 0. Live state at authoring (verified 2026-06-13)

| Fact | Value |
| --- | --- |
| Swarm `main` HEAD | `ba3440ed` (`docs: add dogfood narrative and agent integration guide (#1669)`) |
| Published latest (crates.io) | `0.3.6` (core/cli/facade, 2026-06-11) |
| Latest source release | `v0.3.6` → tag commit source `37637129` (the #534 merge); GitHub Release "v0.3.6 — CLI-truthfulness and evidence-intake patch" (Latest, not prerelease) |
| Current source `main` HEAD | `b7250ff2` (after the #535 publication-receipt PR landed on top of the tag commit) |
| Source publication receipt | source PR #535 → advanced source `main` to `b7250ff2` |
| Source-sync checkpoint (`policy/source-sync.toml`) | `acknowledged_source_main = b7250ff296d2cce1f7aeb1423edc56e25aa4c406`, `acknowledged_by = docs/handoffs/2026-06-11-source-0.3.6-publication-sync.md` |
| Crate versions in tree (swarm) | core/cli/facade all `0.3.6` (`crates/*/Cargo.toml` line 3); `Cargo.lock` all `0.3.6` (lines 923/932/943) |
| `CHANGELOG.md` swarm state | `## Unreleased` is **empty** (line 12); `## 0.3.6 - 2026-06-11` is the top dated section (line 14) |
| Source repo | `EffortlessMetrics/unsafe-review` (local remote `public`) |
| Swarm repo | `EffortlessMetrics/unsafe-review-swarm` (local remote `origin`) |
| Local clone has both remotes | `origin` = swarm (ssh), `public` = source (ssh) — promotion can run from this one checkout |

> **Tag vs. main, do not conflate:** `37637129` is the **v0.3.6 tag commit** (the #534 release merge). Current source `main` has moved one commit past it to `b7250ff2` (the #535 receipt PR). The mirror checkpoint tracks `b7250ff2`. Reconcile from `b7250ff2`, not from `37637129`.

The empty `## Unreleased` means **the session's work is not yet recorded in `CHANGELOG.md`**. The 0.3.7 prep PR must author the dated 0.3.7 section from the session-merged PRs listed in §1; there is no pre-staged `Unreleased` block to rename. (This differs from prior releases where swarm accumulated `Unreleased` entries during the lane.)

### Authoritative docs to read before executing (all in-repo, verified present)

- `docs/contributing/SWARM_TO_MAIN.md` — promotion routing, eligibility, history-catch-up vs curated.
- `docs/contributing/SOURCE_HISTORY_CATCHUP.md` — the swarm→source **history-preserving merge-commit** model (never squash).
- `docs/releases/CRATES_IO_PATCH_RELEASE.md` — versioning, validation, publish order, smoke, receipt.
- `docs/contributing/SWARM_MIRROR.md` — the source→swarm **squash** mirror-back and CHANGELOG convention.
- Prior precedents to model on: `docs/releases/0.3.6-cli-truthfulness.md`, `docs/releases/0.3.6-publication-receipt.md`, `docs/handoffs/2026-06-11-release-0.3.6-preparation.md`, `docs/handoffs/2026-06-11-source-0.3.6-publication-sync.md`.

---

## 1. Release framing

**0.3.7 — adoption, telemetry, and real-code low-noise improvements.**

0.3.7 makes `unsafe-review` easy to adopt and measured in use, and tightens how findings are framed and ranked on real code. It adds **no analyzer breadth** and **no new claim**: it remains advisory static unsafe-coverage evidence. It ships the post-0.3.6 adoption + telemetry lane plus a batch of framing/selection and live-Action correctness fixes — published 0.3.6 predates all of this, so nothing in this lane reaches external users until 0.3.7 publishes.

### Included changes (grounded in the session-merged PRs — all verified MERGED)

Adoption surface:
- Composite **GitHub Action** for two-line adoption — wraps `first-pr`, uploads the review kit, appends the GitHub summary to the step summary, exposes the gate manifest; advisory by default with no posting/blocking/inherited-fail (SPEC-0037, #1628). Lives at `.github/actions/unsafe-review-first-pr/action.yml`. Action correctness + published-version-pin alignment, `fetch_depth` input, and live-smoke version-skew fixes (#1660, #1661, #1662, #1663, #1664).
- **External-first peak-RSS posture** ratified: ADR-0008 amended so resource measurement lives on the scheduled/bench path, not in-tool (#1627). The scheduled full-corpus backstop + external RSS harness (SPEC-0039, #1631).
- Zero-config **`unsafe-review pr`** with repo/base autodetect — no silent full-repo scan, no ambiguous default blocking (#1629; `pr` is wired to `parse_first_pr` in `crates/unsafe-review-cli/src/parse.rs`).
- Adoption front-door `docs/START-HERE.md` (#1635) and outward-facing docs: the real-world dogfood narrative and the agent-integration guide (#1669).

Telemetry / measurement:
- Low-noise **usefulness telemetry** projected from `ReviewCard` — cards/PR, new/worsened/resolved/inherited, selected vs not-selected reasons, agent-ready vs human-only (SPEC-0038, #1630). Enrichment: `scan_cost` (elapsed_ms/output_bytes), not-selected-class histogram, unfulfilled-obligation count (#1634). Register the `usefulness-telemetry.json` artifact kind in the review-kit classifier (#1647). Boundary: this is a **subset/usage signal, not calibrated precision/recall**.

Truthfulness / framing / selection (the real bugs were framing + selection, not detection):
- All output surfaces agree on agent readiness: added `RequiresWitnessReceipt` to agent/LSP readiness so coverage no longer claims "agent-ready" while the comment plan says `requires_witness_receipt` (#1633, resolving issue #1632).
- Comment-plan **importance ranking** (top-N by priority → gap-severity → confidence → file/line, not file order) (#1645) and a **bounded comment body** (≤220 words, single-sourced constant) (#1646).
- **`pub(crate)`/`pub(super)` visibility** framing fix (#1666) and **spread-aware selection** so a cap no longer blinds whole subsystems (#1667).
- Telemetry artifact-kind classifier fix so real bundles validate (#1647).

Corpus / doctrine (internal evidence, not user-facing claims):
- Corpus-usefulness rollup and real-world findings rollups (#1637), doctrine and corpus regression cases (#1638, #1639, #1640, #1641, #1642), improved coverage-movement symmetry (#1643), validation closeout (#1636).
- Internal learnings encoded into specs/status (#1668) and outward-facing docs (#1669).

### AVOID list (wording that breaks the trust boundary — must not appear anywhere in release notes, CHANGELOG, GitHub Release body, receipt, or commit subjects)

Do **not** frame 0.3.7 as, or let any prose imply:
- a UB detector / "finds UB" / UB-free,
- "zero false positives" / calibrated precision/recall / accuracy %,
- memory-safety proof / "proves" anything sound,
- Miri-clean (unless a matching witness receipt is attached),
- site / site-execution proof,
- a policy gate / default blocking / default comment-posting,
- witness execution by the tool / silent source edits.

The telemetry is a **usage/subset signal**, not a precision metric. The Action is **advisory**, never a default gate. Keep the standing boundary sentence from prior receipts verbatim where a boundary line is required.

---

## 2. Ceremony — ordered steps

> All steps run from the local swarm clone (the checkout that carries both `origin` = swarm and `public` = source remotes). Commands are shown in the repo's `rtk`-prefixed style per `AGENTS.md`. The controller has **not** run any of these.

### 2.1 Sync posture (read-only; controller could run, but did not — listed for the owner)

```bash
rtk cargo run --locked -p xtask -- source-divergence
```
Expect: current source `main` = `b7250ff2` (= `acknowledged_source_main`), v0.3.6 tag commit = `37637129`, swarm `main` = `ba3440ed`, `new_source_commits = 0` (source has not moved since the 0.3.6 receipt PR), and swarm-only commits = the post-0.3.6 lane to promote. If `new_source_commits` is nonzero, a source-only change landed after `b7250ff2` — reconcile it before promotion (do not bulldoze it).

### 2.2 **[OWNER]** History-preserving swarm → source promotion

Per `docs/contributing/SOURCE_HISTORY_CATCHUP.md` and the 0.3.6 precedent (source PR #534): a real merge commit, **never squashed**.

```bash
rtk git fetch public
rtk git fetch origin main
rtk git switch -c release/0.3.7-adoption-telemetry public/main   # branch off SOURCE main (b7250ff2)
rtk git merge --no-ff origin/main                                 # import swarm main ba3440ed; resolve conflicts toward swarm reviewed state
```
- Record parentage for the receipt: previous source `main` parent `b7250ff2`, swarm `main` parent imported `ba3440ed`.
- Resolve conflicts deliberately (the 0.3.6 import hit `CHANGELOG.md` and `docs/handoffs/README.md` — expect the same here). Prefer swarm for reviewed product state. **Do not re-introduce the duplicate `## 0.3.4` CHANGELOG section** that was removed during 0.3.6 — keep source's CHANGELOG single-sectioned.
- Verify no unresolved markers:
  ```bash
  rtk proxy git diff --check
  rtk rg "^<<<<<<<|^=======|^>>>>>>>" -n
  rtk proxy git merge-base --is-ancestor origin/main HEAD   # swarm main must be reachable
  ```

### 2.3 **[OWNER]** Version bump 0.3.6 → 0.3.7 (on the release branch, in source)

Bump all three published crates and the lockfile. **Foot-gun:** bump **only** the three workspace crates and do not let any unrelated transitive dependency move in `Cargo.lock` — re-pin or revert any incidental dep version change a `cargo` regen introduces, and diff `Cargo.lock` to confirm only the three `unsafe-review*` entries changed.

- `crates/unsafe-review-core/Cargo.toml` line 3: `0.3.6` → `0.3.7`
- `crates/unsafe-review-cli/Cargo.toml` line 3: `0.3.6` → `0.3.7` (and its dependency on `unsafe-review-core` if pinned to `0.3.6`)
- `crates/unsafe-review/Cargo.toml` line 3: `0.3.6` → `0.3.7` (and its dependency on `unsafe-review-cli`/`-core` if pinned)
- `Cargo.lock`: the three `name = "unsafe-review*"` entries (lines ~923/932/943) → `0.3.7` (regenerate via a `cargo check` or edit the three lines; do not let other deps move).

Confirm exactly three crate versions changed:
```bash
rtk rg "^version = " crates/*/Cargo.toml
rtk proxy git diff -- Cargo.lock        # expect ONLY the three unsafe-review* version lines
```

### 2.4 **[OWNER]** CHANGELOG + release notes + prep handoff (in source)

- **CHANGELOG.md**: add a dated `## 0.3.7 - 2026-06-13` section under `## Unreleased` (keep `## Unreleased` empty on top). Author entries from §1 (Added/Changed/Fixed grouped); cite issue/PR numbers; carry the standing advisory-boundary sentence. Source-side authoring is required here because swarm's `## Unreleased` is empty — there is no block to rename.
- **`docs/releases/0.3.7-adoption-telemetry.md`**: model on `docs/releases/0.3.6-cli-truthfulness.md`. Lead with the framing sentence (§1), an itemized "What changed" grounded in the merged PRs, and a verbatim **Trust boundary** section (advisory; no proof/UB-free/Miri-clean/site-execution/calibrated/policy-readiness; no witness execution/comments/source edits/blocking).
- **`docs/handoffs/2026-06-13-release-0.3.7-preparation.md`**: model on `docs/handoffs/2026-06-11-release-0.3.6-preparation.md`. Record scope, "what this release carries," import parentage (`b7250ff2` ← `ba3440ed`, `--no-ff` merge), and the prep-only boundary.
- Update `docs/handoffs/README.md` index (newest first).

---

## 3. Validation gate checklist (run from SOURCE, on the release branch, before publish)

> Run from the release branch after §2.2–§2.4. `cargo package` refuses a dirty tree, so commit the prep first. The controller did **not** run these (shared target-dir contention hazard); they are the owner's pre-publish gate.

Core gate:
```bash
rtk cargo fmt --all --check
rtk cargo check --workspace --all-targets --locked
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --locked
rtk cargo run --locked -p xtask -- check-pr
```
(Workspace denies `unwrap_used`/`expect_used`/`panic`/`unsafe_code`; `check-pr` bundles docs/policy/fixtures/calibration/dogfood/spec gates. Swarm CI capacity-skips full Rust lanes, so clippy debt can hide on main — the local gate is the real check.)

Package surface — confirm each crate packs the expected file set (0.3.6 packed core 157 / cli 33 / facade 9 files per the 0.3.6 receipt; large deltas warrant a look):
```bash
rtk cargo package -p unsafe-review-core --list
rtk cargo package -p unsafe-review-cli  --list
rtk cargo package -p unsafe-review      --list
```

Publish dry-run — **with the documented publish-order caveat**:
```bash
rtk cargo publish -p unsafe-review-core --dry-run   # SUCCEEDS standalone
rtk cargo publish -p unsafe-review-cli  --dry-run   # EXPECTED to fail pre-publish
rtk cargo publish -p unsafe-review      --dry-run   # EXPECTED to fail pre-publish
```
> **Caveat (documented, not a mystery):** `cli` and `facade` dry-runs resolve their `unsafe-review-core`/`unsafe-review-cli` dependencies against crates.io, where `0.3.7` does not exist until §4 publishes it. Their dry-runs **will fail on registry resolution** until core/cli `0.3.7` are live. This is the exact condition recorded in the 0.3.6 receipt ("cli/facade dry-run resolved after core was published"). Treat only the **core** dry-run as a hard pre-publish signal; cli/facade are validated by their real publish in §4 and the post-publish install smoke.

Also run a release-surface smoke before publish (matches `SOURCE_HISTORY_CATCHUP.md`):
```bash
rtk cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/raw_pointer_alignment \
  --diff fixtures/raw_pointer_alignment/change.diff \
  --out-dir target/unsafe-review-0.3.7-prepublish-smoke
rtk cargo run --locked -p xtask -- check-first-pr-artifacts target/unsafe-review-0.3.7-prepublish-smoke
rtk cargo run --locked -p unsafe-review -- support
```

---

## 4. **[OWNER]** Publish + tag + GitHub Release + receipt + mirror-back

### 4.1 **[OWNER]** Merge the release branch to source `main`

Per the history-catch-up runbook, merge with a **merge commit** (do not squash, do not rebase):
```bash
gh pr create --repo EffortlessMetrics/unsafe-review --base main \
  --head release/0.3.7-adoption-telemetry \
  --title "release: prepare 0.3.7 adoption-telemetry" --body "<parentage + history-preserving note>"
gh pr merge <PR> --repo EffortlessMetrics/unsafe-review --merge --delete-branch
```
The PR body must state: history-preserving import of swarm `main` `ba3440ed`; previous source parent `b7250ff2`; "must be merged with a merge commit — do not squash or rebase." Record the new source `main` SHA.

### 4.2 **[OWNER]** Publish in dependency order (irreversible)

```bash
rtk cargo publish -p unsafe-review-core    # publish FIRST
rtk cargo publish -p unsafe-review-cli     # after core 0.3.7 is on crates.io
rtk cargo publish -p unsafe-review         # facade LAST
```
crates.io publishes are **irreversible** (yank-only). Publish core, let the sparse index settle, then cli, then facade — this is why the cli/facade dry-runs failed in §3.

### 4.3 **[OWNER]** Install smoke from crates.io

```bash
rtk cargo install unsafe-review --version 0.3.7 --locked --root target/install-published-0.3.7
target/install-published-0.3.7/bin/unsafe-review --version    # expect: unsafe-review 0.3.7
target/install-published-0.3.7/bin/unsafe-review doctor
target/install-published-0.3.7/bin/unsafe-review first-pr --out x   # expect exit 2, "--out-dir" suggestion, no bundle (0.3.6 #531 still holds)
target/install-published-0.3.7/bin/unsafe-review pr --help          # zero-config pr present (#1629)
target/install-published-0.3.7/bin/unsafe-review support
```
Then a first-pr artifact smoke per `docs/releases/CRATES_IO_PATCH_RELEASE.md` §Publish (build a temp fixture, run `first-pr --out-dir`, validate with `xtask check-first-pr-artifacts`).

### 4.4 **[OWNER]** Tag + GitHub Release

```bash
rtk proxy git tag v0.3.7 <source-main-SHA>     # the §4.1 merge commit
rtk proxy git push public v0.3.7
gh release create v0.3.7 --repo EffortlessMetrics/unsafe-review \
  --title "v0.3.7 — adoption, telemetry, and real-code low-noise improvements" \
  --notes-file docs/releases/0.3.7-adoption-telemetry.md --latest
```
Tagging and the Release are **irreversible record events** (`SOURCE_HISTORY_CATCHUP.md` forbids rewriting release tags). Keep the title within the AVOID list — no proof/UB/calibrated/gate framing.

### 4.5 **[OWNER]** Source publication receipt

Write `docs/releases/0.3.7-publication-receipt.md` (model on `docs/releases/0.3.6-publication-receipt.md`): source `main` SHA + parentage (`b7250ff2` ← `ba3440ed`), tag `v0.3.7`, GitHub Release URL, crates.io `core/cli/facade 0.3.7` with packed file counts, install/doctor/first-pr/pr/support/first-pr-artifact smoke results, known limits, and the verbatim trust boundary. Land it as a source PR (the 0.3.6 receipt was source PR #535). Record the resulting source `main` SHA — it becomes the new mirror checkpoint.

### 4.6 **[OWNER]** Mirror-back into swarm

Per `docs/contributing/SWARM_MIRROR.md` (source → swarm = **squash** PR; absorption is tracked by `policy/source-sync.toml`, not git ancestry):

```bash
rtk git fetch public
rtk git switch -c sync/source-0.3.7-publication origin/main
rtk proxy git checkout public/main -- \
  Cargo.lock \
  crates/unsafe-review/Cargo.toml crates/unsafe-review-cli/Cargo.toml crates/unsafe-review-core/Cargo.toml \
  docs/releases/0.3.7-adoption-telemetry.md \
  docs/releases/0.3.7-publication-receipt.md \
  docs/handoffs/2026-06-13-release-0.3.7-preparation.md
```
Then:
- **CHANGELOG.md (surgical, not wholesale):** swarm's `## Unreleased` is currently empty, so add a fresh dated `## 0.3.7 - 2026-06-13` section authored to **content-parity** with source's dated section, add the release intro paragraph, and leave a fresh empty `## Unreleased` on top. Verify parity with the `awk` diff in `SWARM_MIRROR.md`. If source carries any duplicated/malformed dated section, do **not** mirror the defect.
- Write `docs/handoffs/2026-06-13-source-0.3.7-publication-sync.md` (model on `2026-06-11-source-0.3.6-publication-sync.md`) and add `docs/handoffs/README.md` index rows.
- Advance the checkpoint in `policy/source-sync.toml`:
  ```toml
  acknowledged_source_main = "<source main SHA after the §4.5 receipt PR>"
  acknowledged_by = "docs/handoffs/2026-06-13-source-0.3.7-publication-sync.md"
  reason = "Source main advanced through the 0.3.7 release PR (history-preserving import of swarm main ba3440ed onto source b7250ff2) and the 0.3.7 publication receipt; crates.io core/cli/facade 0.3.7, tag v0.3.7, GitHub Release 2026-06-13; install smoke confirmed. Mirrors release metadata; makes no safety/proof/policy-readiness claim."
  ```
- Validate, then open the squash PR:
  ```bash
  rtk cargo run --locked -p xtask -- check-source-sync     # expect new_source_commits=0
  rtk cargo run --locked -p xtask -- source-divergence     # expect new_source_commits=0
  rtk cargo fmt --all --check && rtk cargo check --workspace --all-targets --locked
  rtk cargo clippy --workspace --all-targets --locked -- -D warnings
  rtk cargo test --workspace --locked && rtk cargo run --locked -p xtask -- check-pr
  ```
  PR title: `sync: mirror source 0.3.7 publication into swarm workbench` (squash). Expect raw-ancestry `raw_swarm_only` to show the mirror commit (~1) — harmless; the checkpoint is the source of truth.

### 4.7 **[OWNER]** Post-release closeouts

- Close the session decision/tracking issues that 0.3.7 resolves — **build on existing issues, do not file duplicates**: #1659 (owner-gated adoption readiness, 5 items — OPEN) and #1665 (dogfood real-crate validation, 5 evidence-backed gaps — OPEN) carry the release-gated items; close/annotate only the parts 0.3.7 actually ships.
- **Already CLOSED (verify and leave closed; no action beyond a confirming note):** #1653 (SPEC-0037 status now `accepted`) and #1651 (`fetch_depth` input present in `.github/actions/unsafe-review-first-pr/action.yml`, lines 47/97).
- **Still OPEN — real follow-up, NOT already-fixed:** #1649 (example workflow artifact list). `.github/examples/unsafe-review-first-pr.yml` already lists `unsafe-review-gate.json` (line 84) but **still lists `usefulness-telemetry.json` (line 93)**, which #1649 asks to remove. Either land the one-line example-workflow fix as part of the 0.3.7 prep or carry #1649 forward — do not record it as resolved.
- **Owner-gated, NOT part of this runbook:** promote the composite Action (at `.github/actions/unsafe-review-first-pr/action.yml`) to the source/public repo + cut the `@v1` tag (curated, separate ceremony, per #1659 item 1); prebuilt release binaries; the model/stance decisions (owner-card suppression, `pub(crate)` obligation model, `target_feature` grouping, SAFETY-comment guard wording). External-PR validation is release-blocked (pre-release validates stale 0.3.6) and Bun-fork work is read-only.
- **Leave alone:** #1620 (in-tool RSS, OPEN/parked — external-first is owner-ratified FINAL; do not unpark). Open dep PRs (#1565, #1390 swarm; #532, #516, #515 source) are not part of 0.3.7 — handle by risk after the release.

---

## 5. Done criteria

0.3.7 is shipped when: source `main` carries the history-preserving merge of swarm `ba3440ed` onto `b7250ff2` + 0.3.7 prep; crates.io shows `unsafe-review-core/-cli/(facade) 0.3.7`; tag `v0.3.7` and the "v0.3.7 — adoption, telemetry, and real-code low-noise improvements" GitHub Release exist (Latest); the install smoke from crates.io passes (`--version 0.3.7`, `doctor`, `first-pr --out` exit 2, `pr --help`, `support`); the source publication receipt is committed; and the swarm mirror-back PR has merged with `source-divergence` reporting `new_source_commits = 0`. Every output and document preserves the advisory trust boundary — no proof, UB-free, Miri-clean, site-execution, calibrated, or policy-gate claim anywhere.
