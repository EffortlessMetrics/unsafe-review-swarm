# Design north star

The canonical statement of why the family is shaped the way it is. The product
specs (UNSAFE-REVIEW-SPEC-0028+), the family map
([sibling-tools.md](sibling-tools.md)), and each repo's mirror implement this;
this doc is the source it all points back to. Mirror it per-repo the way the
family doc is mirrored.

## The one-line model

```text
fast deterministic sensors  →  emit receipts  →  orchestrators compile them
   (unsafe-review, ripr,           one receipt        ub-review  = LLM review layer
    cargo-allow, tokmd, …)         envelope           cockpitctl = deterministic merge surface
```

Deterministic sensors are instruments. An orchestrator is the gate. The LLM
reviews on top of the deterministic evidence; it never becomes the evidence.

## Boundaries (the moat)

- **Deterministic decides the merge; the LLM is advisory.** The mandatory
  deterministic floor is the only hard gate. LLM availability, cost, or opinion
  never blocks a merge.
- **Every claim is deterministic or labelled a hypothesis.** No proof, UB-free,
  Miri-clean, site-execution, calibrated-precision, or policy-ready claim. This
  discipline is the product's moat and it is under constant pressure as the LLM
  layer gets cheaper and more capable — institutionalize it in rails, do not
  leave it to memory.
- **No NIH.** Build a sensor only to fill a real, uncovered gap; otherwise
  compose a good existing tool (the family already runs many third-party
  sensors). The shared receipt envelope lets first-party and third-party
  sensors sit in one merge surface.

## The inverted cost model (why this works now)

Deterministic work is the **cheap** thing that hides under the **expensive**
thing — the LLM investigation latency is the long pole, and the runner CPU is
idle during it. So the full deterministic suite runs *concurrently, under* the
LLM window for ~free; it does not compete with it. Cheap, long-context models
make multi-step LLM review the **per-PR default**, not a rationed exception.

Anchor: MiniMax-M3 at ~$0.60/M, 1M context, with caching ≈ **$0.50/PR** at ~2
runs. The 1M context is the spec that matters most — this task is context-bound
(review a diff against all the grounded evidence at once), not reasoning-bound,
so a cheap-but-capable model that holds everything is the right fit; you do not
need a frontier reasoner.

The move is not "AI is cheap." It is **"AI makes selectivity cheap"** — you pay
a cheap model to decide which deterministic checks deserve runner time, instead
of running the heavy pile every PR.

## What makes the thesis hold — and the receipts that prove it

The $0.50/PR economics are valid **only while the loop stays bounded,
cache-amplified, single-runner, and receipted.** Drop any of those and it
breaks: the 1M window is the *aperture, not the budget* (at $0.60/M, $0.50 buys
~833k tokens — so the window must be mostly cached precontext with a bounded
fresh payload of diff + new artifacts), two runs/PR keeps spend bounded, and the
deterministic floor must keep finishing *inside* the LLM window. The standing
risk is the **half-life**: every new sensor wants to be mandatory, and the floor
silently grows from "free under latency" into the long pole.

The defense is **receipts** — they make "selectivity is cheap" auditable instead
of a story, and they own the boundary cleanly:

- **unsafe-review owns only the floor.** `unsafe-review-gate.json` (SPEC-0034)
  exposes `required_floor_wall_seconds` + artifact pointers; it never becomes the
  cost accountant.
- **ub-review owns the cost/quality story** (it owns the LLM lanes, caching,
  runners, and fill selection): a per-run cost receipt that *composes* the floor
  wall-time, a suggested-fill ledger (did the model's compute-allocation choices
  find signal?), a floor-time release trend (the half-life alarm), and quality
  telemetry (at $0.50/PR, cost won't warn you when quality drifts — a separate
  signal must). Tracked in ub-review #336–#339.

## The CI gate

One tight gate, one runner, one required check:

- **Tight required list** — the deterministic floor that always runs and blocks.
  Keep it genuinely tight; it is the one thing the time budget cannot save you
  from, and its wall-time must be tracked over releases (the floor accretes).
- **Longer structured suggested list** — a catalog of available, non-mandatory
  checks.
- **LLM fills the runner** from the suggested list, and **devises PR-specific
  checks beyond it** from the diff + repo context — all advisory, all
  budget-bounded (configurable target 30 min / cap 60 min).
- **Security rail** for devised execution: no secrets in the devised-execution
  environment, read-only command allowlist, provenance + logging. Plan it
  reasonably; it is normal untrusted-input hygiene, not a crisis.

Reference implementation: [unsafe-review-swarm #1524]. Onboarding/migration:
`ub-review init` / `align` (ub-review #330) — ingest a bloated CI, classify each
check (load-bearing → required; coverable/advisory → suggested;
confirmed-redundant → drop; **unknown → stays required, flagged**), and propose
a reviewable diff, never a silent overwrite.

## The flywheel

Cheap grounded models on every PR → issues + fixes → better sensors → better
grounding → more found. The grounding (deterministic floor + receipts) is what
makes it compound instead of spinning into noise — which is where ungrounded
"AI for CI" dies. The standing question is only whether **coherence keeps pace
with velocity**.

## Open frontiers (watch these)

- **One receipt envelope, owned.** The highest-leverage convergence: every
  sensor emits one envelope both orchestrators consume. Anchor on the proven
  side (ub-review's real consumption, ripr's shipped `gate-decision.json`);
  cockpitctl (early-alpha) and the rest align to it. Give it a DRI, not just
  cross-linked issues.
- **Validate on a repo you did not build.** Self-dogfooding (this repo,
  ub-review) optimizes for a clean distribution; the real test is the **Bun UB
  hunt** — a large, foreign, messy repo. Weight that above more self-runs.
- **Floor-time budget half-life.** The "free under latency" economics hold only
  while the deterministic floor finishes inside the LLM window. Emit the floor's
  wall-time each run so the trend is visible before a PR suddenly takes 25
  minutes.
- **Recursive-dogfood escape hatch.** ub-review gating ub-review's own PRs is
  fine *only* because the deterministic floor runs independently — ub-review's
  gate must never be the only way to validate ub-review.
- **Release ceremony via shipper.** Promote → publish → tag → receipt → mirror
  is the last hand-cranked ceremony; run it through `shipper` (resumable,
  backoff-aware) so a release can never half-happen (0.3.2 was version-bumped
  but never published).
- **Framing is the IP.** The judgment in this doc is the org's most valuable,
  most tacit asset. Encode it everywhere (per-repo mirrors) and centrally (here).

[unsafe-review-swarm #1524]: https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/1524
