# Dogfood control: 2026-05-26 no-card fixture smoke

Status: false-positive control report
Artifact status: local, untracked under `target/unsafe-review-no-card-control-smoke/`

This report records a no-card control run for the existing
`fixtures/safe_code_no_cards` fixture. It is a small false-positive control for
the dogfood lane, not a real-crate corpus target and not a calibration sample.

It does not prove the repository safe, UB-free, Miri-clean, or that any unsafe
site executed. It does not run witnesses, post comments, edit source, or enable
blocking policy.

## Scope

Control target:

- `fixtures/safe_code_no_cards`

Commands:

```bash
rtk cargo run --locked -p unsafe-review -- first-pr \
  --root fixtures/safe_code_no_cards \
  --diff fixtures/safe_code_no_cards/change.diff \
  --out-dir target/unsafe-review-no-card-control-smoke

rtk cargo run --locked -p xtask -- check-first-pr-artifacts \
  target/unsafe-review-no-card-control-smoke
```

Observed result:

```text
Review cards: 0
Open actionable gaps: 0
No changed unsafe-review gaps were found.
This does not prove the repo safe, UB-free, Miri-clean, or that any unsafe site executed.
check-first-pr-artifacts: ok
```

## Triage observations

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `fixtures/safe_code_no_cards` | zero-card first-pr bundle | `actionable` | The fixture emitted zero cards and the generated bundle preserved honest no-card wording. | Keep this as a false-positive control; do not promote it to real-crate precision evidence. |

## Trust boundary

This report is static advisory review evidence for one no-card fixture smoke.
It is not memory-safety proof, UB-free status, Miri-clean status, site-execution
proof, calibrated precision, calibrated recall, witness adequacy, release
readiness, or policy readiness.
