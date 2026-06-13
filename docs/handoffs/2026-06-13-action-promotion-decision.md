# Action Promotion Decision: composite Action -> `EffortlessMetrics/unsafe-review@v1`

Date: 2026-06-13
Audience: owner (release authority)
Status: PREP / decision packet — no tag move, no source mutation performed by this document.

Verified live state at draft time: swarm `main` `ba3440ed`; published crates.io version `0.3.6` (confirmed via crates.io versions API — latest published is `0.3.6`, no `0.3.7`); latest source release `v0.3.6` (2026-06-11); no `v0.3.7`, `v1`, or `v1.*` tag exists in the source repo; source repo `EffortlessMetrics/unsafe-review`; swarm repo `EffortlessMetrics/unsafe-review-swarm`.

---

## Problem

External Rust repositories want to adopt `unsafe-review` PR coverage via a one-line `uses:` reference. The published adoption surface documented today is `uses: EffortlessMetrics/unsafe-review@v1` (see `docs/ci/github-action.md` lines 12, 40, and `.github/actions/unsafe-review-first-pr/action.yml` line 21). That `@v1` reference does **not yet resolve** — the composite Action currently lives only in the development repo at `.github/actions/unsafe-review-first-pr/action.yml`, which is explicitly marked "not the published surface" (`action.yml` line 23; `docs/ci/github-action.md` lines 160–166; SPEC-0037 §2 lines 55–57).

The owner needs a decision on **how** and **when** to make the `@v1` reference real in `EffortlessMetrics/unsafe-review`, without (a) breaking the advisory trust boundary, (b) creating a floating `@v1` that silently changes behavior under external callers, or (c) promoting before the install pin and the artifact contract are validated against the version `@v1` will actually install.

This packet documents the current contract verbatim, lays out the tag-strategy options, and recommends a sequence. It builds on the already-open owner decision issues **#1659** (owner-gated adoption readiness — item 1 is "Promote composite action to EffortlessMetrics/unsafe-review", release-ceremony, depends on the smoke-test PR #1652) and **#1665** (dogfood real-crate validation findings). It does **not** duplicate them.

---

## Current contract (verbatim from repo files)

### Trust boundary (fixed, on every surface)

From `action.yml` lines 12–14 and SPEC-0037 §1 lines 34–37, identically:

> Static unsafe contract review only. Not memory-safety proof, not UB-free status, not Miri-clean status, and not site-execution proof.

### Advisory posture — what the Action does NOT do

From SPEC-0037 §7 lines 138–146 and `action.yml` lines 7–10, the Action does NOT:

- post PR comments
- run Miri, `cargo-careful`, sanitizers, Loom, Shuttle, Kani, or Crux (no witness execution)
- edit source files
- enforce blocking policy on `unsafe-review` findings by default
- request write permissions on `GITHUB_TOKEN`
- claim memory safety, UB-free status, Miri-clean status, or site-execution proof
- upload artifacts itself (caller controls upload — SPEC-0037 §6 lines 132–134)

The minimum caller permission block is `contents: read` (`docs/ci/github-action.md` lines 149–154; SPEC-0037 §7 line 148). The Action "must never receive or use write tokens for comment posting, source editing, or branch mutation" (SPEC-0037 §7 lines 155–156).

`unsafe-review` is the evidence layer; `ub-review` is the orchestrator that reads `unsafe-review-gate.json` and decides/posts (SPEC-0037 §10 lines 205–207). Promotion does not change that split.

### Install default — pinned version 0.3.6

`action.yml` `inputs.version` defaults to `"0.3.6"` (lines 41–46) and installs via `cargo install unsafe-review --locked --version "${UNSAFE_REVIEW_VERSION}"` (line 107). SPEC-0037 §3 lines 61–64 confirms `cargo install --locked --version <pin>` from crates.io is the MVP acquisition path; default `0.3.6` is "the latest published crates.io version" and the pin "prevents silent breakage on new releases." No pre-compiled release binaries are distributed yet (SPEC-0037 §3 lines 66–69; `docs/ci/github-action.md` lines 136–145).

### Exit-code semantics (advisory by default)

From `action.yml` lines 117–136:

- Exit 0 — advisory pass, including advisory findings (not a failure).
- Exit 1 — policy result (new/worsened diff-scoped gaps). Fails the job **only** when `fail_on_new_debt: true`; otherwise passes through. Inherited/baseline gaps never count (SPEC-0037 §4 lines 89–92, §10 lines 194–196).
- Exit 2 — tool error (malformed input / internal failure) — always fails.

`fail_on_new_debt` defaults to `false` (`action.yml` lines 61–68; SPEC-0037 §4 line 87).

### Bundle artifacts — required-core vs optional `if-present` (version-skew rule)

The "Verify bundle file shape" step (`action.yml` lines 138–177) splits artifacts into two tiers:

- **Required core** (hard-required, must exist and be non-empty; `action.yml` lines 145–159): `review-kit.json`, `cards.json`, `pr-summary.md`, `github-summary.md`, `cards.sarif`, `comment-plan.json`, `witness-plan.md`, `receipt-audit.md`, `manual-candidates.json`, `manual-repair-queue.json`, `tokmd-packets.json`, `lsp.json`, `repair-queue.json`, `unsafe-review-gate.json`.
- **Optional, verified only if present** (`action.yml` lines 170–177): `usefulness-telemetry.json` — checked only when emitted; a present-but-empty file is still an error.

The governing rule is SPEC-0037 §11 (lines 216–227): the Action installs the **pinned published** version, so its required-artifact verify must match what **that** version emits, not the dev-tree tip. Artifacts added to the dev tree after the pinned release must be `if-present`, not hard-required — hard-requiring a new artifact would fail the Action for callers on the old published version until a new release ships it. "Shipping dev-tree improvements to action users requires a new release — the fix is a release, not a weaker verify."

### Live smoke — passed against published 0.3.6

SPEC-0037 §12 (lines 230–235) lists the proof commands; §13 lines 239–249 records lifecycle status **Accepted (2026-06-13)** with all criteria met, including: "A live end-to-end smoke test has run green via `smoke-action.yml` (`workflow_dispatch`, run 27456120005 on `main`, against the published binary): the action produced the bundle, appended the job summary, and set its `bundle_dir` and `gate_status` outputs. The smoke verify is version-skew resilient." The smoke workflow itself (`.github/workflows/smoke-action.yml`) is `workflow_dispatch`-only, `contents: read`, never `pull_request`/`push`, and asserts the two outputs are non-empty (lines 18–22, 46–61).

**Key facts for the decision:**

- The green smoke proves the Action runs against **published 0.3.6**. It does not yet prove anything against 0.3.7 (unpublished) or against any external (non-swarm) repository.
- The smoke job checks out with `fetch-depth: 0` (full history; `smoke-action.yml` line 38). It therefore does **not** exercise the Action's `fetch_depth` default (100) or shallow-fetch base-ref-resolution path. That path remains unproven in CI — see Risk 3.

---

## Options

### Option A — exact-tag-only (promote, reference a specific tag/SHA, no `@v1`)

Promote the Action into `EffortlessMetrics/unsafe-review`, then have docs/examples reference an immutable tag or SHA (e.g. `uses: EffortlessMetrics/unsafe-review@v1.0.0` or `@<sha>`). No floating `@v1` ref is created.

- Pros: callers pin to an immutable surface; no silent behavior drift; the safest first promotion; lets one real external PR exercise the Action before a floating ref exists.
- Cons: callers must bump the tag to get fixes; the documented two-line `@v1` snippet (`docs/ci/github-action.md` lines 12, 40) stays aspirational until the floating ref is created; slightly higher adoption friction.

### Option B — `@v1` now (create the floating major tag immediately against current state)

Promote and immediately create/point `@v1` at the current Action (which installs published `0.3.6`).

- Pros: matches the docs verbatim today; lowest adoption friction; nothing in the contract is wrong against `0.3.6` (smoke is green).
- Cons: `@v1` is a floating ref — every future change to the Action under `EffortlessMetrics/unsafe-review` re-points it under all external callers with no opt-in. Creating it before 0.3.7 publishes means the next release + version-default bump moves under callers untested. The version-skew rule (SPEC-0037 §11) is only proven for `0.3.6`; a `@v1` that later defaults to `0.3.7` is unvalidated at tag-creation time.

### Option C — `@v1` after 0.3.7 is published AND the Action smoke passes against 0.3.7, with one external-PR validation first (recommended)

Sequence:
1. Promote the Action source into `EffortlessMetrics/unsafe-review` (history-preserving, per #1659 item 1).
2. Reference it by exact tag/SHA first (Option A mechanics) for any uncertainty.
3. Publish `0.3.7`; bump the Action `version` default to `0.3.7`; re-run `smoke-action.yml` and confirm green against published 0.3.7 (closes the version-skew loop in SPEC-0037 §11 for the new default).
4. Run the Action against at least one real **external** PR (a non-swarm Rust repo) to validate the cross-repo path — fetch-depth ancestor resolution, `contents: read` only, artifact shape, outputs set.
5. Only then create/point `@v1`.

- Pros: `@v1` is only created once the install default, the artifact contract, and the smoke are all green against the version `@v1` will install; the floating-ref blast radius is validated before external callers depend on it; honors the SPEC-0037 §11 doctrine ("the fix is a release") and #1659's release-ceremony framing.
- Cons: more steps; `@v1` is delayed until after the 0.3.7 ceremony; requires one external-repo validation run to be arranged.

### Option D — `@v1` against current published 0.3.6, freeze the Action, defer further edits

A middle path: promote, point `@v1` at the current `0.3.6`-pinned Action now (Option B mechanics), but treat the promoted `action.yml` as frozen — no edits land in `EffortlessMetrics/unsafe-review` until a deliberate, smoke-validated re-tag. This bounds the floating-ref blast radius by policy rather than by sequencing.

- Pros: satisfies the documented `@v1` snippet immediately; the freeze discipline contains drift.
- Cons: the freeze is a soft promise, not a mechanism — any later edit (including the eventual `0.3.7` default bump) still moves `@v1` under callers; defers rather than closes the version-skew and cross-repo validation gaps. Strictly weaker than Option C on the two risks that matter, with no real adoption-speed advantage over Option A for cautious adopters who can pin a tag.

---

## Recommendation

**Adopt Option C.** Do **not** move `@v1` until 0.3.7 is published AND the Action smoke passes green against 0.3.7. Until that point, if there is any uncertainty, use an **exact version tag/SHA first** (Option A mechanics) so early external adopters pin to an immutable, smoke-proven surface.

Rationale grounded in the contract:
- The only green live proof today is against **published 0.3.6** (SPEC-0037 §13 lines 244–249). A floating `@v1` created now is correct for 0.3.6 but becomes an untested promise the moment the default bumps to 0.3.7.
- The version-skew rule (SPEC-0037 §11) explicitly ties artifact-verify correctness to the **pinned published** version. Creating `@v1` before re-validating against 0.3.7 would float the major ref ahead of its own proof.
- #1659 already frames promotion as an owner release-ceremony (history-preserving merge + tag), dependent on the smoke-test PR. Option C is the disciplined execution of that already-owner-gated item, not a new decision lane.

The trust boundary, advisory defaults, no-write-token posture, and no-comment/no-blocking/no-witness-execution contract are unchanged by any option — promotion is a hosting/tag decision, not a semantic change. Do not let promotion become an excuse to alter exit semantics, comment posting, or witness execution.

---

## Risks

1. **Floating-`@v1` blast radius** — once `@v1` exists in `EffortlessMetrics/unsafe-review`, every future Action edit re-points it under all external callers with no opt-in. Mitigation: Option C delays `@v1` until validated; document that callers may pin a tag/SHA for stability.
2. **Version-skew on default bump** — bumping the Action `version` default from `0.3.6` to `0.3.7` without re-running the smoke could hard-require an artifact the new default does not emit, or vice versa (SPEC-0037 §11). Mitigation: re-run `smoke-action.yml` against published 0.3.7 before `@v1`; keep newly-added dev-tree artifacts in the `if-present` tier (`action.yml` lines 170–177) until a release ships them.
3. **Cross-repo / fetch-depth path unproven** — the only smoke run was on the swarm repo itself (run 27456120005 on `main`), and it checked out with `fetch-depth: 0`, so it bypassed the Action's shallow `git fetch --depth` base-ref resolution entirely (`action.yml` lines 93–100; `fetch_depth` default 100, lines 47–54). External repos with deeper PR histories may hit ancestor-resolution edge cases the default does not cover. Mitigation: Option C step 4 (one external-PR validation), deliberately on a repo whose PR history exceeds a shallow clone.
4. **Crates.io install latency / availability** — `cargo install` cold-start (~45s, per #1659 item 2) and any crates.io outage become caller-facing failures once `@v1` is live; pre-compiled release binaries are not yet distributed (SPEC-0037 §3). Mitigation: document `Swatinem/rust-cache@v2` (already in `docs/ci/github-action.md` lines 38, 139–145); track the prebuilt-binary path under #1659 item 2 — out of scope for this tag decision.
5. **Trust-boundary erosion under external eyes** — external adopters may misread the advisory bundle as a UB/memory-safety/Miri result. Mitigation: the fixed-footer trust boundary is already on every surface (`action.yml`, SPEC-0037, `docs/ci/github-action.md` lines 110–121); do not soften it during promotion.
6. **Dogfood-known gaps surface externally** — #1665 documents real, advisory-quality gaps (spread-unaware card selection under `--max-cards`; `pub(crate)` mis-reported as public API). These are characterization findings (explicitly NOT calibrated precision/recall, NOT a safety claim — #1665 body), not contract violations, but a wider audience will see them. Mitigation: track under #1665; not a promotion blocker, but argues for the 0.3.7-first sequence so any landed fixes ship under `@v1`.

---

## Acceptance

`@v1` may be created in `EffortlessMetrics/unsafe-review` once all of the following hold:

- [ ] **[OWNER]** Action source promoted to `EffortlessMetrics/unsafe-review` via history-preserving merge (per #1659 item 1).
- [ ] `0.3.7` published to crates.io and confirmed installable via `cargo install unsafe-review --locked --version 0.3.7`.
- [ ] Action `version` default bumped to `0.3.7` (`action.yml` line 46) and the matching docs/spec references updated (`docs/ci/github-action.md` lines 14, 43, 61; SPEC-0037 §3 line 63, §4 line 84).
- [ ] `smoke-action.yml` re-run via `workflow_dispatch` and **green** against published `0.3.7` (bundle produced, job summary appended, `bundle_dir` + `gate_status` outputs set); run id recorded in SPEC-0037 §13.
- [ ] Required-core vs `if-present` artifact tiers reconciled against what `0.3.7` actually emits (SPEC-0037 §11; `action.yml` lines 145–177) — no dev-tree-only artifact is hard-required.
- [ ] At least one **external** (non-swarm) Rust PR exercised through the promoted Action by exact tag/SHA, confirming: `contents: read` only, no comments posted, no write token used, advisory exit 0 on findings, artifact shape intact, and base-ref resolution succeeds on a history deeper than a shallow clone.
- [ ] Trust boundary, advisory defaults, and no-write-token posture verified unchanged on the promoted copy (diff the promoted `action.yml` against swarm `ba3440ed`).
- [ ] **[OWNER]** Create/point `@v1` (release-ceremony tag step). **This is the only step that creates the floating major ref; do not perform it until every box above is checked.**

Until `@v1` exists, external adopters should reference the promoted Action by exact tag or SHA.

### Out of scope for this packet (do not bundle into the tag decision)
- Prebuilt release binaries (#1659 item 2) — separate owner call.
- Blocking-policy implementation (#1659 item 3) — `fail_on_new_debt` stays advisory-opt-in.
- Dogfood detection-quality fixes (#1665) — advisory quality, tracked separately.

### Companion-issue status (verified against the live tracker)
- **#1653** — "SPEC-0037 status -> accepted": **already CLOSED.** SPEC-0037 reads `Status: accepted` (line 3) and §13 records Accepted (2026-06-13). No action needed.
- **#1651** — "add `fetch_depth` input": **already CLOSED.** The input is present in `action.yml` lines 47–54. No action needed.
- **#1649** — "fix example workflow artifact list": **still OPEN, but substantively satisfied.** The concern is the *example workflow* `.github/examples/unsafe-review-first-pr.yml` (not `docs/ci/github-action.md`). Its verify loop (lines 70–99) now includes `unsafe-review-gate.json` in the required-core list (line 84) and treats `usefulness-telemetry.json` as optional/if-present (lines 92–99), and the upload step carries `if: always()` (line 106). The originally-reported verify mismatch is resolved; this issue is closeable on a maintainer check — do not block this decision on it.

---
*This packet is a decision aid only. It performs no tag move and no source mutation. Every box marked [OWNER] requires the release authority to act.*
