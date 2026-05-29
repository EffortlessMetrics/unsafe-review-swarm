# Swarm Digest: <date or range>

Date:
Owner:
Swarm range:
Source checkpoint:

Use this template to summarize a batch of `unsafe-review-swarm` work after it
lands. A digest is a workbench handoff, not a release receipt, support-tier
promotion, calibration claim, policy decision, or source-promotion request.

## Queue state

| Repo | Open PRs | Notes |
|---|---:|---|
| `unsafe-review-swarm` |  |  |
| `unsafe-review` |  |  |

Source divergence:

```bash
rtk cargo run --locked -p xtask -- source-divergence
```

Observed result:

- <observed result>

## Landed swarm PRs

| PR | Commit | Surface | Reason this mattered | Validation |
|---|---|---|---|---|
| #<n> | `<sha>` | analyzer / dogfood / projection / CI / docs |  |  |

## ReviewCard and projection impact

- ReviewCard identity:
- Evidence applicability:
- Dogfood or fixture signal:
- Projection surfaces touched:
- Verifier rails added:

## Dogfood observations

When an observation creates follow-up work, add or update the matching row in
`docs/dogfood/follow-up-seeds.md`. Keep seed IDs stable enough to survive
digest-to-PR handoff, and do not leave actionable dogfood pressure only in
free-form prose.

| Target | Observation | Triage label | Seed ID | Seed status | Source report | Follow-up |
|---|---|---|---|---|---|---|
| `<target>` |  | `actionable` / `noise` / `missed` / `needs-fixture` / `needs-doc` / `needs-route` / `needs-analyzer` / `needs-verifier` | `dogfood-...` | `open` / `done` / `parked` / `superseded` | `docs/dogfood/reports/...` |  |

## Validation

Record only commands actually run for this digest:

```bash
rtk cargo fmt --check
rtk cargo check --workspace --all-targets --locked
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --locked
rtk cargo run --locked -p xtask -- check-pr
rtk cargo run --locked -p xtask -- source-divergence
rtk git diff --check
```

Observed result:

- <observed result>

## Source promotion posture

- [ ] not a promotion candidate yet
- [ ] future curated promotion candidate
- [ ] source sync acknowledgement needed
- [ ] source promotion PR prepared separately

Reason:

- <reason>

## Known limits

- <limit>

## Trust boundary

This digest records swarm workbench evidence only. It does not prove memory
safety, UB-free status, Miri-clean status, site execution, calibrated precision,
calibrated recall, release readiness, or policy readiness. It does not imply
witness execution, automatic comments, source edits, or default blocking policy.

## Next narrow slices

1. <next slice>
2. <next slice>
3. <next slice>
