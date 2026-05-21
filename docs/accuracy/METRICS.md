# Accuracy metrics and claim tiers

## Core metrics

- `inventory_recall`
- `operation_family_accuracy`
- `hazard_precision`
- `hazard_recall`
- `evidence_precision`
- `evidence_recall`
- `false_positive_control_pass_rate`
- `route_agreement`
- `identity_stability`
- `artifact_honesty`

## Calibration tiers

| Tier | Meaning | Public claim allowed |
|---|---|---|
| `fixture-pinned` | Curated fixture/golden behavior is locked. | "Fixture-backed for this pattern." |
| `dogfood-measured` | Run on selected real repos/PR diffs. | "Dogfood-measured on selected corpus." |
| `labeled-calibrated` | Human-adjudicated denominator and metrics exist. | "Calibrated for this scoped claim." |
| `policy-eligible` | Repeated reports + thresholds + drift checks. | "Eligible for opt-in policy evaluation." |
| `blocking-ready` | Separate policy spec and release proof. | "May be considered for blocking use." |

`blocking-ready` is explicitly out-of-scope for this lane.
