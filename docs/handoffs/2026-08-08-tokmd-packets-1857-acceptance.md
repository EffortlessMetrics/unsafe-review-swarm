# Issue #1857 tokmd packet acceptance

Date: 2026-08-08

This receipt qualifies the current producer/consumer path for issue #1857. It
does not publish either tool, modify the tokmd-swarm repository, or claim that
the advisory packet content proves memory safety, UB absence, Miri cleanliness,
or site execution.

## Candidate identity

- Swarm producer commit: `488f41550a5c8770c8c16f923b3af51593b51039`
  (`docs: record issue 1889 recovery audit`, PR #2015).
- Producer packages: `unsafe-review-core`, `unsafe-review-cli`, and
  `unsafe-review`, all `0.3.8`.
- Consumer repository: `EffortlessMetrics/tokmd-swarm` at
  `3d278c56d4afe37583e67500fc2e89e60c3077fe`.
- Consumer package: `tokmd` `1.15.0`.
- Source divergence on the producer commit: `new_source_commits=0`.
- Qualification environment: Linux workspace checkout, Rust 1.95 toolchain,
  local current-main builds. No crates.io or published-binary claim.

## Producer receipt

The producer command was run with six committed manual-candidate examples and
a representative raw-pointer fixture diff:

```text
unsafe-review first-pr --root <bundle> --diff <bundle>/change.diff --out-dir <bundle>/out
```

The resulting `tokmd-packets.json` reports:

- `schema = "tokmd.packets/v1"`;
- compatibility `schema_version = "tokmd-packets/v1"`;
- `renderer.tokmd_run = false`;
- all five named presets in `renderer.available_presets`;
- six packets, six manual candidates, and zero analyzer-discovered packets;
- all five `preset_inputs` keys plus the bundle trust-boundary input.

Packet manifest SHA-256:

```text
0d03108e7144c23fc30a4dee8c16fce124bc421bd120faefc5f3e631c6902ca9
```

The producer remains formatting-input only: it does not run tokmd, execute
witnesses, post comments, edit source, or enforce a blocking policy.

## Consumer receipt

`cargo test --locked -p tokmd --test render_packets_integration` passed all 7
tests on tokmd-swarm current main. This includes schema rejection, absent-input
limitation, unknown-preset rejection, sibling-input ingestion, and packet
rendering.

The generated producer bundle was then passed through each required consumer
preset. All five commands exited successfully and tokmd's CLI boundary accepted
the canonical packet schema before rendering:

| Preset | Output SHA-256 |
| --- | --- |
| `bun-ub-handoff` | `780b156766469e18d4fcebd4e19356eeef8bc863ce2fb4762f21f485aaa08cb7` |
| `bun-ub-pr-body` | `6c31662642e4130cc8e358dc3ffd4225b19f1f4c852dfeb429a5dfb745246ce2` |
| `bun-ub-ledger-note` | `7fdf9ee93a7bb6dbffda6d94477a15aa513ee76943cf2cf94e4593d922002b80` |
| `bun-ub-review-map` | `34786c860de9881463af3c82a6fbc9a606f1c06c86a1a0d1c0a359025b543091` |
| `bun-ub-next-pick` | `dacc329bd9ca9646d8d993979afbfcfa60be36aff3d164b7101fca5649610011` |

The rendered outputs preserve explicit limitations. The review-map output also
preserves the explicit no-posting boundary and reports its missing
`comment-plan.json` selection summary rather than presenting an all-clear.

## Disposition

The current producer/consumer contract is proven for the named candidate bundle
and five presets. This is the release-shape receipt for #1857; it is not a
global tokmd compatibility claim. The issue remains available for maintainer
closeout or any narrower follow-up discovered from a different packet shape.
