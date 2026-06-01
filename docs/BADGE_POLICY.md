# Badge policy

`unsafe-review` badges are public evidence signals, not safety claims.

## Principles

- Every badge is an advisory evidence indicator.
- Badge rows summarize public status surfaces; they do not certify analyzer correctness.
- No badge implies memory safety, soundness, UB-freedom, or Miri execution.
- Every public badge maps to checked-in endpoint JSON or an explicitly planned
  surface. README badges must not be inferred from ungenerated artifacts.

## Meaning table

| Badge | Meaning | Not meaning |
|---|---|---|
| CI | Current GitHub CI status. | Analyzer correctness proof. |
| `unsafe-review` | Numeric open static review gap count. | Safety or unsafety status. |
| `unsafe-review+` | Numeric repair-plus-quality count: open review gaps plus missing-or-weak evidence findings. | Miri-clean or UB-free status. |
| VS Code planned | Editor surface is planned. | Published VS Marketplace extension. |
| Open VSX planned | Editor surface is planned. | Published Open VSX extension. |
| GitHub release | Latest published GitHub release tag. | crates.io availability or release quality proof. |
| crates.io downloads | Public crates.io download count. | Adoption quality or safety proof. |
| docs.rs | Current docs.rs build badge. | API stability guarantee. |
| MSRV | Declared minimum supported Rust version. | Toolchain-wide compatibility guarantee. |
| License | Declared project license expression. | Legal advice. |

## Generation contract

Badge endpoints are repo-scoped static evidence projections from ReviewCards.
They are not safety badges.
They must be generated from the CLI through the core serde-backed badge
renderer and covered by repository checks.

- Generate endpoint JSON: `cargo run --locked -p unsafe-review -- badges --out badges/`
- Verify core badge rendering: `cargo test -p unsafe-review-core badge_json --locked`
- Verify badge behavior: `cargo test -p unsafe-review --test e2e repo_inventory_and_badges_count_open_gaps_without_safety_claim --locked`
- Verify public endpoint allowlist: `cargo test -p xtask generated_artifact_detector_is_narrow --locked`
- Verify checked-in endpoints match the current repo projection: `cargo test -p xtask public_badge_endpoints_match_generated_repo_projection --locked`
- Run the repository gate: `cargo run --locked -p xtask -- check-pr`

Endpoint badges may appear in README rows only when `badges/unsafe-review.json`
and `badges/unsafe-review-plus.json` are checked in and covered by the
validation path above.

The checked-in public endpoints must be Shields endpoint JSON only. They may
contain `schemaVersion`, `label`, `message`, `color`, and other
Shields-supported presentation fields, but they must not contain internal
repo-contract fields such as `contract_version`, `kind`, `scope`, `basis`,
`status`, or `counts`.

`unsafe-review` uses the open actionable review gap count as its numeric
message. `unsafe-review+` uses open review gaps plus missing-or-weak evidence
findings as its numeric message. The meaning belongs in this policy document
and the badge link target, not in the endpoint message.

## Forbidden badge posture

Badge labels, messages, docs, and endpoint JSON must not say or imply:

- safe or all clear
- sound
- verified
- UB-free
- Miri-clean
- policy-ready or blocking-ready
- site execution

Those words may appear only in explicit negative trust-boundary wording.
