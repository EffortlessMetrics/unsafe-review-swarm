# Dogfood report: 2026-05-28 arrayvec first-pr projection smoke

Status: focused projection smoke report
Swarm commit: `c5b581a`
Artifact status: local, untracked under `target/dogfood-work/`

This report runs the `arrayvec-pr288` dogfood target through the full
`first-pr` advisory bundle after the evidence-applicability and projection
rails landed. The goal is to check whether a real PR diff produces a small
review cockpit and coherent artifacts, not to add analyzer breadth.

It is not a support-tier promotion, calibration report, policy decision,
safety proof, UB-free claim, Miri-clean claim, witness result, or
site-execution proof. No witness tools were run.

## Scope

Target:

- `arrayvec-pr288`

Command:

```bash
rtk cargo run --locked -p unsafe-review -- first-pr \
  --root target/dogfood-work/arrayvec \
  --diff target/dogfood-work/arrayvec-pr288.raw.diff \
  --out-dir target/dogfood-work/arrayvec-pr288.first-pr-smoke
```

Artifact verification:

```bash
rtk cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/dogfood-work/arrayvec-pr288.first-pr-smoke
```

## Summary

| Surface | Result | Reviewer note |
|---|---:|---|
| `cards.json` | 8 cards, 8 open actionable gaps | 7 `guarded_unwitnessed`, 1 `unsafe_unreached`; no `guard_missing` remains for the `try_push_str` `set_len(new_len)` card. |
| `pr-summary.md` | Verified | Shows a top `vec_set_len` card with missing witness evidence, Miri route, explain command, context command, and trust boundary. |
| `github-summary.md` | Verified | Bounded doorway summary only; it does not include the full card table or witness plan. |
| `comment-plan.json` | Verified | 0 planned comments; all 8 cards are accounted for in `not_selected` with low relevance and threshold reasons. |
| `witness-plan.md` | Verified | Routes `vec_set_len` cards to Miri/cargo-careful style evidence and the unsafe function call to human deep review. |
| `lsp.json` | Verified | Saved editor projection remains read-only and card-derived. |

## Top card

```text
ID: UR-arrayvec-src-array-string-rs-from-byte-string-operation-vec_set_len-set-len-073a0fa631f6-initialized_memory-c1
Class: guarded_unwitnessed
Location: src/array_string.rs:140
Operation: vec.set_len(CAP);
Operation family: vec_set_len
Missing evidence: No witness receipt imported for this card
Primary route: miri
Next action: Attach a focused Miri or cargo-careful witness receipt or mark the static limitation explicitly.
```

The top-card projection is useful: it tells the reviewer that static capacity
and initialization evidence were found, but that witness evidence is still
absent. The next action is a receipt or an explicit static limitation, not a
claim that the card proves the `set_len` path sound.

## Comment-plan observation

`comment-plan.json` selected no inline comments. The cards are medium-priority
and medium-confidence, and the report keeps them available in artifacts rather
than spending reviewer attention through planned comments.

That is the intended low-noise behavior for this target: the PR packet is still
useful, but the future trusted-comment lane would stay quiet by default.

## Triage observation

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `arrayvec-pr288` | first-pr projection bundle | `actionable` | The bundle verifies across cards, Markdown summaries, SARIF, comment-plan, witness-plan, and saved LSP, while keeping comments planned-only and bounded. | Re-run this smoke after future PR-summary, comment-budget, saved-LSP, or agent-context projection changes. |

## Trust boundary

This is static unsafe contract review dogfood. It does not prove memory safety,
UB-free status, Miri-clean status, unsafe-site execution, witness adequacy,
release readiness, or policy readiness. It is not calibrated precision or
calibrated recall evidence.
`unsafe-review` did not run witnesses, post comments, edit source, or enforce
blocking policy.
