# Product-stance decision packets — owner gate

Date: 2026-06-13
Status: decision packets, prep-only (no implementation)
Author role: research / packet prep
Audience: owner (decision authority)

## Why this document exists

Three dogfood-exposed product-stance questions need an owner decision before any code change. They surfaced from the real-crate validation pass and are already tracked as evidence-backed gaps in:

- [#1665](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1665) — dogfood real-crate validation findings (5 evidence-backed gaps). Items #3 (`unknown` family volume), #4 (target_feature explosion), and #5 (SAFETY-comment-present guard_missing) map directly onto the three packets below.
- [#1659](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1659) — owner-gated adoption readiness. These three are *semantic-stance* gates, distinct from the release-ceremony / blocking-policy / LSP items in #1659; they belong in the same owner-gated bucket but are not duplicated there.
- [`docs/dogfood/REAL_WORLD_FINDINGS.md`](../dogfood/REAL_WORLD_FINDINGS.md) — narrative characterization. The "rigor is the product" tension (§"'Noise feel' is often disagreement with correct strictness") and the reclassification-not-resolution finding both bear on packet 3.
- [`docs/specs/UNSAFE-REVIEW-SPEC-0029-unsafe-evidence-coverage-model.md`](specs/UNSAFE-REVIEW-SPEC-0029-unsafe-evidence-coverage-model.md) — already states the owner-card-arithmetic position (§"`unknown` operation-family volume is owner-card arithmetic", lines 95-109) and names suppression-policy (not classifier change) as the volume lever. Packet 1 builds on that spec rather than re-deciding it.

This document does not introduce new claims. It builds on the above and ends each packet with an explicit owner-decision gate. Nothing here implements owner-card, target_feature, or SAFETY-comment semantic changes.

> Note on #1665 fix suggestions: items #3 and #5 in the issue propose, respectively, "extend the classifier" and "a contract-present-but-unverifiable tier." Packet 1 deliberately takes the SPEC-0029 stance (volume is owner-card arithmetic; the lever is a suppression *policy*, not a classifier change), and Packet 3 treats the new tier as the heavier, optional follow-up rather than the default. Where these packets diverge from the issue's casual fix wording, the packet is the considered position and supersedes the issue text.

### Trust boundary (applies to all three packets)

`unsafe-review` is advisory static unsafe-contract review: it finds unsafe Rust changes missing a safety contract, guard, test, or witness. None of the options below may add memory-safety-proof, UB-free, Miri-clean (absent a matching witness receipt), site-execution, or calibrated precision/recall wording, and none may suppress *evidence* to manufacture a lower noise number. Grouping/ranking changes operate on the comment-plan surface; the ReviewCard set and JSON artifact stay the single truth object.

---

## Packet 1 — OWNER-CARD surfacing (unsafe-fn owner-card volume)

### Problem

On real unsafe-heavy crates, per-declaration `unsafe fn` owner cards dominate raw card counts and read as noise. The question is whether to keep, suppress, group, budget, or re-tier them. This is *owner-card arithmetic* (one card per `unsafe fn` declaration), not a classifier defect.

### Observed evidence (dogfood)

- #1665 item #3: the `unknown` operation family dominates — 338 of ~1000 cards on `memchr`, 250+ on `crossbeam` — with "could not infer" obligations, and these weak, human-deep-review-only cards crowd out better-characterized cards.
- SPEC-0029 §"`unknown` operation-family volume is owner-card arithmetic" already states this volume is per-declaration owner cards representing the caller's contract obligation for a function body, by design.

### Current behavior (cite code)

- `analysis/evidence/reach_scan.rs:60` emits reach state `"owner_reached"` for the declaration-level owner card — the unit is the `unsafe fn` decl, not a contained operation.
- `analysis/classify.rs:36-49`: an owner card with no contract → `ContractMissing` (High/High); with a contract but no discharge → `GuardMissing` (High/Medium). Owner cards carry `OperationFamily::Unknown`.
- **Comment-plan already de-prioritizes owner cards.** `output/comment_plan/selection.rs:174-180` (`should_plan_comment`) excludes `OperationFamily::Unknown` entirely:
  ```rust
  && !matches!(card.operation.family, OperationFamily::Unknown)
  ```
  and `selection.rs:157-160` (the `NOT_SELECTED_UNKNOWN_FAMILY_REASON` constant) / `:187-188` (its use in `non_selection_reason`) route any unknown-family card to `not_selected` with `code: "human_deep_review_only"`, message `"operation family unknown"`. The test `comment_plan_skips_unknown_operation_family_cards` (`output/comment_plan/mod.rs:500-561`, fixture `public_unsafe_fn_missing_safety`) proves an `unknown`-family owner card with class `contract_missing` is never selected for an inline comment and lands in `not_selected` with `reason: "operation family unknown"` and `reason_code: "human_deep_review_only"`.
- **Verification of the importance-ranking layer (merged PR [#1645](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1645), "comment-plan: rank candidates by importance"):** the comment-plan importance ranking + 3-comment budget (`selection.rs:28-47` `importance_rank`; `selection.rs:95-98` `MAX_COMMENT_BUDGET_REASON` = "comment-plan max of three candidates reached"; dedup-by-family/obligation at `selection.rs:91-94` `OPERATION_FAMILY_BUDGET_REASON`) is a *second* layer of de-prioritization, but it never runs on owner cards because the `unknown`-family gate in `should_plan_comment` excludes them before ranking. So owner cards are already fully suppressed *from the comment surface* by an eligibility gate, not merely down-ranked by budget. The residual problem is artifact/raw-count volume (JSON, SARIF, badge `+` counts), not comment noise.

### Options

- **A — Keep as-is.** Owner cards stay one-per-decl in every surface. Pro: zero evidence loss; matches SPEC-0029 stance. Con: raw artifact counts stay high on unsafe-heavy repos; badge `+` count looks noisy.
- **B — Keep artifacts, suppress-or-group in comment-plan (recommended).** Evidence stays complete in JSON/SARIF; the comment-plan continues to exclude owner cards (already true) and, additionally, *groups* owner-card volume in the human/comment-plan summary so the count is presented as "N owner-contract cards (see artifacts)" rather than implied noise. No card deletion.
- **C — Suppress owner card when an operation card already covers the same region.** Per SPEC-0029 §lever ("suppress owner cards that are already fully covered by their contained operation cards — an owner/policy decision, not a classifier change"): an owner card whose body is already fully covered by contained-operation cards is suppressed (in artifacts, with a recorded suppression reason). Stronger noise reduction, but it removes a card from the truth surface and needs a precise "fully covered" definition to avoid hiding an uncovered obligation.
- **D — Owner-card budget.** Cap owner cards per file/per scan with an explicit "+N more owner cards" overflow marker. Bounds volume without per-region semantics; risks capping a genuinely distinct decl.
- **E — New importance tier.** Add an owner-contract importance tier so owner cards sort below operation cards everywhere (badges, ordering) without suppression. Heaviest; touches the coverage/importance model in SPEC-0029.

### Recommendation

**B, building toward C as a policy (not classifier) follow-up.** Keep the evidence in artifacts; tighten/group the comment-plan and human-summary presentation. Do **not** delete evidence to fake a lower noise number. The comment surface already excludes owner cards (verified above), so **explicit comment-plan suppression of owner cards is not needed** — what is needed is summary *grouping/labeling* so the artifact count is not misread as comment noise. If the owner later wants raw-count reduction, route it through the SPEC-0029 suppression-policy lever (option C) as an owner/policy decision, never as a classifier change.

> Challenge to the recommendation: because the comment surface is *already* clean (owner cards never reach inline comments), option B reduces to a presentation/labeling change in the human and comment-plan summaries — it does not reduce JSON/SARIF/badge counts at all. If the owner's actual pain is the badge `+` number or artifact size on unsafe-heavy repos, B will not move it and the owner should weigh C (covered-region suppression) or D (budget) directly. B is the right *first* step only if the pain is "summary reads as noise"; it is insufficient if the pain is "raw counts are too high." The packet should make the owner choose which pain they are solving.

### Risks

- C/D can hide a genuinely uncovered obligation if "covered" or the cap is defined loosely — a correctness-over-noise regression.
- E expands the coverage/importance model and the projection-parity surface (SPEC-0029 proof obligations); larger blast radius.
- Any option that changes counts must keep badge/JSON/comment-plan projections mutually consistent (single-truth-object rule).

### Non-goals

- Extending the operation-family classifier to "resolve" `unknown` (SPEC-0029 explicitly says volume is not a classifier gap).
- Deleting owner cards from the JSON/SARIF truth surface.
- Any blocking, posting, or proof/UB-free claim.

### Acceptance if implemented (option B)

- JSON/SARIF artifact owner-card set unchanged (no evidence loss; verified by `check-first-pr-artifacts`).
- Comment-plan continues to exclude owner cards (existing test `comment_plan_skips_unknown_operation_family_cards` stays green).
- Human/comment-plan summary presents owner-card volume as a labeled group, not as implied inline-comment noise.
- No new overclaiming wording; trust-boundary text intact.

**DECISION REQUIRED — owner.**

---

## Packet 2 — `target_feature` grouping (SIMD attribute explosion)

### Problem

SIMD code emits large numbers of structurally identical `#[target_feature]` cards across architecture variants. Decide whether to group them and at which surface.

### Observed evidence (dogfood)

- #1665 item #4: `memchr` emits 1000+ structurally-identical `target_feature` cards across arch variants; proposed fix is group/dedup by pattern.
- REAL_WORLD_FINDINGS.md table lists `memchr` as the crate exercising "SIMD target-feature contracts" — the highest-density target_feature source in the corpus.

### Current behavior (cite code)

- `analysis/target_feature.rs:1-4` detects `#[target_feature(...)]` attributes and `target_feature(...)` call-name forms; each detected arch-variant declaration becomes its own site. (The per-arch-variant fan-out is not provable from `target_feature.rs` alone — it depends on upstream site emission — but the #1665 item #4 evidence of 1000+ near-identical cards on `memchr` backs the volume claim empirically.)
- `analysis/evidence/target_feature_discharge.rs` and `domain/operation.rs` / `domain/hazard.rs` carry the target_feature operation/hazard — one card per detected site, so N arch variants of the same routine produce N near-identical cards.
- Comment-plan dedup at `output/comment_plan/selection.rs:91-94` (`OPERATION_FAMILY_BUDGET_REASON`, "covered by selected family/obligation sibling") already collapses *same-family/same-obligation* siblings to one comment and caps at three (`selection.rs:95-98`). So the comment surface is bounded; the explosion is in the artifact/raw-card surface.

### Options

- **A — Keep as-is.** One card per arch variant everywhere. Pro: each variant is individually addressable. Con: artifact counts balloon on SIMD-heavy crates.
- **B — Group in comment-plan first, preserve per-site artifacts (recommended).** Present grouped target_feature representation in the comment-plan/human summary; keep every per-site card in JSON/SARIF. Mirrors the existing family/obligation dedup behavior already in `selection.rs`.
- **C — Artifact-level dedup by pattern.** Collapse structurally identical variants into one card with a variant list in artifacts too. Strongest count reduction; loses per-site addressability and per-variant baseline/suppression identity.
- **D — Representative-per-pattern with overflow marker.** Keep one representative card plus an "+N variants" note in artifacts.

### Recommendation

**B — group in comment-plan first, preserve per-site artifacts.** Consistent with packet 1's evidence-preservation stance and with the existing comment-plan family/obligation dedup. Artifact-level dedup (C/D) is a separate, heavier decision because per-site cards carry independent identity used by baselines and suppressions.

> Challenge to the recommendation: same caveat as Packet 1 — the comment-plan family/obligation dedup at `selection.rs:91-94` likely *already* collapses same-family/same-obligation target_feature siblings into a single comment, so B's comment-surface benefit may already be in place and B again reduces mostly to summary labeling. If so, the live decision is really A vs C vs D on the artifact surface. Confirm whether the dedup key currently treats arch variants as the same obligation before assuming B adds comment-surface value.

### Risks

- Grouping key must be precise: two variants that differ in their actual obligation (e.g. different required feature set) must not be silently merged, or a real gap is hidden.
- Artifact-level dedup (C) would change card identity (`UR-...-cN`), which SPEC-0029 lists as a non-goal for the coverage model — out of scope here.

### Non-goals

- Removing per-site target_feature cards from artifacts.
- Changing card identity.
- Any claim that grouping proves the SIMD contract discharged.

### Acceptance if implemented (option B)

- Per-site target_feature cards remain in JSON/SARIF (verified by artifact gates).
- Comment-plan / human summary shows a grouped target_feature representation rather than N near-identical entries.
- Grouping key documented; differing-obligation variants are not merged.
- Trust-boundary wording intact; no proof claim added.

**DECISION REQUIRED — owner.**

---

## Packet 3 — SAFETY comment present but guard unverified (wording vs new tier)

### Problem

A `SAFETY:` / `# Safety` comment is present, but no executable guard discharges the obligation. These cards are correct (a comment is a contract statement, not a guard) yet read as false positives to reviewers who treat the comment as sufficient. Decide whether to fix this with wording or with a new readiness tier.

### Observed evidence (dogfood)

- #1665 item #5: `guard_missing` with a `SAFETY:` comment present — 120 of 207 cards on `smallvec`; some are "legit-but-flagged." (The issue's own proposed fix names a "contract-present-but-unverifiable tier" — i.e. option C below; this packet treats that as the heavier optional follow-up, not the default.)
- REAL_WORLD_FINDINGS.md §"'Noise feel' is often disagreement with correct strictness": explicitly states "a `SAFETY` comment does not discharge an obligation… A reviewer who expects the comment to satisfy the card will call it noise." This is documented as a *correct* card with a clear next step, not a detection defect — i.e., a wording/framing problem, not a classifier problem.

### Current behavior (cite code)

- `analysis/classify.rs:36-49`: when contract is present (`contract.present == true`, i.e. a SAFETY comment was found) but discharge is absent (`!discharge.present`), the card is classed `GuardMissing` (Priority::High, Confidence::Medium). This is the exact "SAFETY-present, guard-unverified" path.
- `analysis/pipeline/action_summary.rs:27-47` — the **generic** `GuardMissing` arm (operation-specific `GuardMissing` arms for `unknown`, `unsafe_fn_call`, `inline_asm`, `pin_unchecked` precede it at lines 23-26) renders the next action as:
  > "Add or expose local guards for these `{operation}` safety obligations: …" / "Add or expose the local guard that discharges the `{operation}` safety obligation."

  This wording does **not** acknowledge that a SAFETY comment is present and explains the obligation, and it does not name the three legitimate discharge routes (executable guard / focused test reach / matching witness receipt) as alternatives.
- The distinction between "SAFETY comment present, guard unverified" and "contract missing entirely" already exists at the *class* level (`GuardMissing` vs `ContractMissing`, `classify.rs:36-49`) and at the SPEC-0029 coverage-slot level (`contract_coverage: present, guard_coverage: missing`). What's missing is reviewer-facing wording that says so.

### Options

- **A — Keep as-is.** No change; rely on class + coverage slots. Con: reviewers keep reading these as false positives.
- **B — Wording-first (recommended).** Refine the `GuardMissing`-with-contract-present `next_action` to acknowledge the present SAFETY text and name the discharge routes, e.g.:
  > "SAFETY text explains the obligation; add executable guard evidence, focused test reach, or a matching witness receipt to discharge it."

  No new class/tier; the comment-≠-guard boundary stays intact (the card still fires).
- **C — New readiness tier `contract_present_guard_unverified`.** Add a distinct tier separating "plausible SAFETY comment present" from "contract missing entirely." Heavier: touches SPEC-0029 coverage vocabulary, projection-parity across surfaces, and calibration entries. (This is the fix #1665 item #5 itself suggests.) Flag as separate/optional follow-up.
- **D — B + a coverage-slot rendering hint** (no new tier): surface the existing `contract_coverage: present / guard_coverage: missing` pair more prominently in human/comment output, reusing slots SPEC-0029 already defines.
- **E — Suppress these cards when a SAFETY comment is present.** Rejected on its face — it would make comment = guard, breaking the core evidence doctrine and the trust boundary.

### Recommendation

**B (wording-first), optionally with D.** Refine the next-action wording so the card explains that the SAFETY text states the obligation and names the three discharge routes, without weakening the "comment ≠ guard" stance. The card must still fire. Treat **C (new `contract_present_guard_unverified` tier) as a separate, optional, heavier follow-up** — it is a coverage-vocabulary and projection-parity change (SPEC-0029) and should not block the wording fix. Reject E.

> Challenge to the recommendation, and an implementation caveat: option B as written would refine *only* the generic `GuardMissing` arm (`action_summary.rs:27-47`). But the operation-specific arms above it (`unknown`, `unsafe_fn_call`, `inline_asm`, `pin_unchecked` at lines 23-26) also fire on `GuardMissing` cards that may have a present SAFETY comment, and they carry their own bespoke wording. If those arms are not also touched, a `smallvec`-style SAFETY-present card whose operation resolves to one of those families will still get the old wording and still read as noise. The acceptance criteria must therefore specify *which* arms B covers, or the fix will be partial on exactly the cards #1665 measured. Recommend B explicitly cover every `GuardMissing` arm that can co-occur with `contract_coverage: present`, or be gated on the coverage slot rather than the operation string.

### Risks

- Wording must not imply the SAFETY comment partially discharges the obligation or that the site is "probably fine" — that would soften the trust boundary.
- A new tier (C) ripples into badges, comment-plan selection reasons, the LLM packet, the gate manifest, and calibration entries (SPEC-0029 §"One projection, many surfaces"); doing it casually risks surface drift.
- Must not introduce any "likely safe" / "no UB" framing.

### Non-goals

- Making a SAFETY comment discharge or suppress the card (rejected option E; breaks comment ≠ guard).
- Lowering priority/confidence purely because a comment exists.
- Any proof / UB-free / site-execution / calibrated-accuracy claim.

### Acceptance if implemented (option B)

- For a `GuardMissing` card with `contract_coverage: present`, the next-action wording acknowledges the present SAFETY text and names the three discharge routes (executable guard / focused test reach / matching witness receipt) — across **all** `GuardMissing` arms that can co-occur with a present contract, not only the generic arm.
- The card still fires (comment ≠ guard preserved); class stays `GuardMissing`; priority/confidence unchanged.
- A fixture/calibration case covers "SAFETY comment present, guard unverified" wording.
- No overclaiming wording; xtask wording gates green.
- If C is pursued later, it is a standalone spec-backed change with its own projection-parity proof.

**DECISION REQUIRED — owner.**

---

## Cross-references and non-duplication

- Builds on #1665 (items #3/#4/#5) and #1659 (owner-gated bucket); does not re-file either. Where the packets diverge from #1665's casual fix suggestions, the divergence is intentional and noted inline.
- Builds on SPEC-0029 (owner-card arithmetic + suppression-policy lever, lines 95-109) and REAL_WORLD_FINDINGS.md (rigor-is-the-product framing); does not restate their conclusions as new.
- All three packets are prep-only. No implementation, no spec mutation, no calibration change is performed here.
