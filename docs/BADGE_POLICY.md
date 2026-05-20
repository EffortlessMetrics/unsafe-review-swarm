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
| Codecov | Uploaded coverage signal. | Test adequacy proof. |
| `ripr+` | Static oracle-exposure evidence. | Mutation testing or runtime mutation confirmation. |
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
They must be generated or checked by xtask.

- Generate endpoint JSON: `cargo run --locked -p xtask -- badges`
- Verify endpoint JSON and README references: `cargo run --locked -p xtask -- badges --check`
- Run the repository gate: `cargo run --locked -p xtask -- check-pr`

Until endpoint JSON exists and passes `--check`, endpoint badges should not be
added to README rows.
