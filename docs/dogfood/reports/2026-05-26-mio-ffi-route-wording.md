# Dogfood report: 2026-05-26 mio FFI route wording

Status: focused follow-up report
Swarm commit: `c9b9f34`
Artifact status: no new dogfood artifact; verifier rail only

This report records the focused follow-up for the `mio-pr1388` FFI/platform
route wording observation from the post-burst analyzer snapshot. The original
snapshot found a useful platform/layout cluster with one `miri_unsupported`
FFI card, but the follow-up was to keep reviewer wording human-review-heavy and
avoid implying Miri coverage.

The follow-up landed as `xtask` verification in swarm PR #477. It requires
`miri_unsupported` fixture next actions to name both:

- the explicit FFI boundary contract; and
- the limitation that Miri may not exercise the seam.

It is not a support-tier promotion, calibration report, dogfood rerun, policy
decision, safety proof, UB-free claim, Miri-clean claim, witness result, or
site-execution proof. No witness tools were run.

## Scope

Target:

- `mio-pr1388`

Original dogfood artifact:

```text
target/dogfood-work/mio-pr1388.after-local-safety-colon.json
```

Verifier evidence:

```bash
rtk cargo test -p xtask fixture_card_identity_rejects_stale_class_next_actions --locked
rtk cargo run --locked -p xtask -- check-fixtures
```

## Summary

| Follow-up | Result | Reviewer note |
|---|---|---|
| FFI route wording | Verifier rail added | `miri_unsupported` next actions must route reviewers to sanitizer/cargo-careful evidence while naming the FFI boundary contract and Miri non-execution limitation. |

## Triage observation

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `mio-pr1388` | FFI/platform layout cluster | `needs-route` | The original snapshot had an appropriate `miri_unsupported` FFI card, but reviewer wording needed to avoid implying Miri coverage. | Swarm PR #477 added a fixture verifier requiring FFI boundary contract wording and explicit Miri limitation text. |

## Trust boundary

This report records a wording invariant. It does not mean sanitizers,
cargo-careful, Miri, or any other witness ran. It does not prove the FFI seam
safe, UB-free, Miri-clean, site-executed, calibrated, or policy-ready.
