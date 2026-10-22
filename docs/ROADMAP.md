# Roadmap

## Swarm internal work program

Swarm-internal roadmap execution continues in
`.rails/lanes/swarm-roadmap/implementation-plan.md`.

This lane is not a publication/release commitment; it tracks workbench
hardening, analyzer validation, dogfood, CI/process alignment, and ReviewCard
projection coherence before any curated promotion to `unsafe-review`.

## 0.1.0 — Static-first review cards

- stable-only source scanner
- review card schema
- CLI: `check`, `repo`, `pilot`, `badges`, `doctor`, `explain`, `context`
- hazard and obligation taxonomy v1
- contract/discharge evidence mining v1
- human / JSON / Markdown output
- specification and policy system

## 0.2.0 — Public usability

- first-run `doctor` readiness check
- one-command `first-pr` / `review` advisory bundle
- readable PR summary, witness plan, and honest no-card states
- reviewer-first `explain <card-id>`
- support posture command
- first-pr artifact bundle verifier
- release target: [0.2.0 public usability](releases/0.2.0-public-usability.md)

## 0.3.0 — Editor-adjacent and agent workflow

- saved-workspace LSP diagnostics
- hover cards
- code actions for copying packets and witness commands
- copy-only bounded agent packets
- live `unsafe-review lsp` server landed (SPEC-0018); extension client wiring
  remains deferred until saved artifacts and first-run UX are solid

## Versioning: staying on 0.x.y

No 1.0 is planned. Minor bumps mark usefulness arcs; patch bumps mark fixes.
A 1.0 would require stability promises this tool does not make: API stability,
default blocking policy, and calibrated precision/recall. Until those exist as
proven claims, every release stays advisory-only under `0.x.y`.

## 0.4.0 — Repo posture and policy (shipped 2026-09-16)

- baseline and suppression matching
- no-new-debt mode
- repo inventory hardening
- badge output hardening
- outcome comparison

Qualified under swarm #1925; publication mirrored in swarm #1879
(`docs/handoffs/2026-09-16-0.4.0-publication.md`).

## 0.5.0 — Usefulness arc (shipped 2026-09-17, crates.io)

Cut from swarm cutline `f281130e` (pre-publish #2250), promoted through
source #570/#571 with the publication receipt in source #572. Latest
crates.io version is 0.5.0; latest GitHub Release is v0.4.0. Post-cutline
fixes (#2251, #2253–#2257) are current-main behavior in `Unreleased`, not
part of the immutable 0.5.0 crates (see CHANGELOG).

- `SAFETY:` doc comments count as contract evidence (#2235)
- inner unsafe-fn sites route to caller-contract review (#2237)
- deref-of-`get_unchecked` folds into the single `get_unchecked` card (#2239)
- unreached witness commands stay behind a test-first cue (#2241)
- `--short` risk-ranked one-line-per-card output (#2245)
- slot-level missing counts in headers (#2243)
- `--short` rejected for the policy report; shipped selector runs through
  Git Bash on Windows (#2249, #2246)
- receipt import for Miri, cargo-careful, sanitizers, Loom, Kani, and Crux;
  witness-plan artifacts; confirmation-cue execution stays opt-in
  (`confirm <card-id> --allow-heavy`)

## 0.6.x — Path forward (planned, still 0.x.y)

- precision measurement on the evidence corpus
  ([swarm #2223](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2223))
- independent accuracy denominator plus first unfamiliar baseline batch
  ([swarm #2224](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2224));
  first measured claim for the `NonNull::new_unchecked` aperture
  ([swarm #2231](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2231))
- AST-authoritative dispatch, first slice `nonnull_unchecked`
  ([swarm #2225](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2225))
- truthful witness execution facts and one structured invocation
  ([swarm #2228](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2228),
  [swarm #2229](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2229))
- subject-bound receipts and comparable outcomes
  ([swarm #2230](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2230))
- source-role-aware action surfacing
  ([swarm #2227](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2227))
- remaining analyzer acceptance: shadowed/reassigned bindings, after-op and
  non-dominating guards, debug/test-reference exclusion. The obligation-key
  split that closed
  [swarm #2226](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2226)
  shipped in #2257; cross-function span flow stays with
  [swarm #1393](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1393)
- agent forward-progress governance
  ([swarm #2221](https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2221))
- fixture-backed calibration corpus
- false-positive tracking
- dogfood-calibrated evidence loop
- saved-snapshot outcome reasons and receipt movement
- non-blocking advisory policy reports
- support-tier promotion rules
- optional nightly/MIR fact adapter ADR
- repo outcome comparison

## Next analyzer work

Recorded as known follow-ups; not claimed as implemented by any current
release.

- stale-span-after-reentry detection: flag a raw pointer/length obtained from a
  JS ArrayBuffer/TypedArray that is used after a call that can re-enter user JS
  (`coerce_to_*`, property access, callback) without re-fetching, re-validating,
  or pinning the span; an `is_detached()` check against a pre-call snapshot is
  the tell
  ([swarm #1393](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1393)).
  A first fixture-pinned advisory heuristic now covers the same-function shape:
  materialize-after-reentry and stale-span-use-after-reentry both emit
  `stable-byte-source-getter-reentry` cards, and a stale pre-reentry
  `is_detached()` check is named in the card instead of counting as a guard.
  Remaining follow-ups: cross-function span flow, helper-returned spans, and
  length-only staleness; the heuristic is syntactic and fixture-pinned, not a
  dataflow proof.
- optional confirmation-cue execution (`--allow-heavy`): the opt-in
  `confirm <card-id> --allow-heavy` command now exists and executes a card's
  routed witness command locally, recording the result only as a saved witness
  receipt through the existing import constructors
  ([swarm #1394](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1394)).
  Confirmation cues remain unexecuted by default; nothing runs without the
  explicit `--allow-heavy` opt-in. Remaining follow-up: rank cards by
  confirmed-vs-pending confirmation state.

## Deferred

- automatic code fixes
- generated tests
- rustc_private/MIR dependency in the product binary
- blocking gate defaults
