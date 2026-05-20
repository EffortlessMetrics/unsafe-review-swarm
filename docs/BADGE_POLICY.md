# Badge policy

`unsafe-review` badges are public evidence signals, not safety claims.

## Principles

- Every badge is an advisory evidence indicator.
- Badge rows summarize public status surfaces; they do not certify analyzer correctness.
- No badge implies memory safety, soundness, UB-freedom, or Miri execution.

## Meaning table

| Badge | Meaning | Not meaning |
|---|---|---|
| CI | Current GitHub CI status. | Analyzer correctness proof. |
| `unsafe-review` | Open static review gaps. | Safety or unsafety status. |
| `unsafe-review+` | Contract/guard/witness gap summary. | Miri-clean or UB-free status. |
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
They must be generated from the CLI and covered by repository checks.

- Generate endpoint JSON: `cargo run --locked -p unsafe-review -- badges --out badges/`
- Verify single-card badge behavior: `cargo test -p unsafe-review --test e2e repo_inventory_and_badges_count_open_gaps_without_safety_claim --locked`
- Verify multi-card badge behavior: `cargo test -p unsafe-review --test e2e repo_badges_follow_multicard_review_card_summary --locked`
- Run the repository gate: `cargo run --locked -p xtask -- check-pr`

Endpoint badges may appear in README rows only when `badges/unsafe-review.json`
and `badges/unsafe-review-plus.json` are checked in and covered by the
validation path above.
