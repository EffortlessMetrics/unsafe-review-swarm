# tokmd consumer five-preset receipt (issue #1857)

Date: 2026-08-10

This is a fresh current-main consumer acceptance receipt for issue #1857. It
re-proves the acceptance criterion — `tokmd render --from-packets` succeeds for
all five Bun presets against producer output from this repository — on
integrated current main, superseding the local-commit receipt audited in the
#1889 recovery (PR #2015). It does not publish either tool, modify the
tokmd-swarm repository, or treat the advisory packet content as proof: not
memory-safety proof, not UB-free status, not Miri-clean status, and not
site-execution proof.

## Candidate identity

- Swarm producer commit:
  `779ded1715bbc323fd7275e8f393f97d6132138c`
  (`docs(release): record swarm #2014 actions-pin integration (#1916) (#2081)`,
  current `origin/main` at receipt time).
- Producer packages: `unsafe-review-core`, `unsafe-review-cli`, and
  `unsafe-review`, all `0.3.8`; local debug build of `cargo-unsafe-review`.
- Consumer: `tokmd` `1.15.0`, installed from crates.io with
  `cargo install tokmd --locked --root <install-root>`.
- Canonical schema: tokmd-swarm `main`
  `crates/tokmd/schemas/tokmd-packets.schema.json`, fetched from
  `https://raw.githubusercontent.com/EffortlessMetrics/tokmd-swarm/main/crates/tokmd/schemas/tokmd-packets.schema.json`,
  SHA-256 `563eb5ae6e30382b1d91341aa61e37a1692803ff4d95a47a1db3d72c88fce7bc`.
- Source divergence on the producer commit: `new_source_commits=0`.
- Qualification environment: Linux workspace checkout, Rust 1.95.0 toolchain,
  local current-main build plus the crates.io tokmd binary. No
  published-unsafe-review-binary claim.

## Producer receipt

The bundle was staged from the committed `fixtures/raw_pointer_alignment`
fixture plus the six committed manual-candidate examples under
`docs/examples/manual-candidates/`, mirroring the first-pr e2e flow:

```text
cp -r fixtures/raw_pointer_alignment <bundle>/fixture
cp docs/examples/manual-candidates/*.json <bundle>/fixture/.unsafe-review/candidates/
cargo-unsafe-review first-pr --root <bundle>/fixture --diff <bundle>/fixture/change.diff --out-dir <bundle>/out
```

The resulting `tokmd-packets.json` reports:

- `schema = "tokmd.packets/v1"` (canonical dotted id landed in PR #1863);
- compatibility `schema_version = "tokmd-packets/v1"`;
- `renderer.tokmd_run = false` with all five named presets in
  `renderer.available_presets`;
- six packets, all `source = manual`, `manual_candidate = true`,
  `analyzer_discovered = false`;
- per-packet `preset_inputs` carrying all five Bun presets plus
  `trust_boundary`;
- honest input presence: bundle-level `inputs` marks `manual-candidates.json`,
  `manual-repair-queue.json`, and `comment-plan.json` as included and
  `cards.json`, `receipt-audit.md`, `repair-queue.json`, `witness-plan.md`, and
  the stable-byte seed ledger as not included for this manual-candidate slice,
  each with a stated limitation; per-packet `missing_inputs` records absent
  evidence rather than an all-clear;
- non-claims surfaced as `claims_not_made` under
  `preset_inputs.bun-ub-pr-body` (not proof of UB, not proof of memory safety,
  not UB-free status, not Miri-clean status, not site-execution proof, not
  calibrated precision or recall, not policy readiness, not automatic repair).

Sibling sidecars `manual-candidates.json` (`schema_version =
manual-candidates/v1`, six candidates) and `cards.json` (`schema_version =
0.2`) are present in the same bundle directory.

Artifact hashes:

| Artifact | SHA-256 | Bytes |
| --- | --- | --- |
| `tokmd-packets.json` | `bb24a144ac692bcf6676bec7ca924d44d6bc4d2946cc21caf98ca1831a4c51dd` | 176432 |
| `manual-candidates.json` | `ad147d313b8aecf0943c503120e94350024581cfc97e68b2c18cf0cfda3876a4` | — |
| `cards.json` | `2b5485f06ec3658dbb9c7c846243f328ef9164aab5cf99a27977e0e707898e2d` | — |

Schema validation: the producer manifest validates against the canonical
schema above (Python `jsonschema` 4.25.1, draft-07 `validate`, no errors). The
schema's only required property is the top-level `schema` const
`tokmd.packets/v1`.

The producer remains formatting-input only: it does not run tokmd, execute
witnesses, post comments, edit source, or enforce a blocking policy.

## Consumer receipt

For each preset:

```text
tokmd render --from-packets <bundle>/out --preset <preset> --output <preset>.md
```

All five commands exited 0 with empty stderr:

| Preset | Result | Output SHA-256 | Bytes |
| --- | --- | --- | --- |
| `bun-ub-handoff` | PASS | `780b156766469e18d4fcebd4e19356eeef8bc863ce2fb4762f21f485aaa08cb7` | 293 |
| `bun-ub-pr-body` | PASS | `6c31662642e4130cc8e358dc3ffd4225b19f1f4c852dfeb429a5dfb745246ce2` | 290 |
| `bun-ub-ledger-note` | PASS | `7fdf9ee93a7bb6dbffda6d94477a15aa513ee76943cf2cf94e4593d922002b80` | 297 |
| `bun-ub-review-map` | PASS | `34786c860de9881463af3c82a6fbc9a606f1c06c86a1a0d1c0a359025b543091` | 516 |
| `bun-ub-next-pick` | PASS | `dacc329bd9ca9646d8d993979afbfcfa60be36aff3d164b7101fca5649610011` | 289 |

The rendered outputs carry explicit limitations rather than an all-clear.
Because the producer emits `preset_inputs` per packet while the tokmd consumer
contract reads top-level manifest `preset_inputs`, each render records the
manifest-level absence explicitly (for example,
"`preset_inputs` for `bun-ub-handoff` is absent from the bundle and no sibling
files supplied derivable sections"), and `bun-ub-review-map` additionally
records that `cards.json` was not ingested and that the `comment-plan.json`
selection summary section is missing, alongside the explicit no-posting
boundary. Per the tokmd render spec this is the designed degradation path:
missing inputs produce an explicit limitation, never an empty or all-clear
document. The output digests match the 2026-08-08 handoff receipt byte-for-byte
because the fixture inputs and renderer are deterministic; the qualifying
difference here is that the producer build is current integrated main.

## Trust boundary

This receipt is consumer acceptance evidence only: the canonical packet schema
parsed at the tokmd CLI boundary, all five preset names were accepted, and
rendered output carried producer limitations and non-claims through. It does
not run witnesses, execute Miri, execute Bun or Node, edit source, or post
comments. It does not claim UB presence or absence: not memory-safety proof,
not UB-free status, not Miri-clean status, not site-execution proof, not
calibrated precision or recall,
merge readiness, policy readiness or a blocking policy, or any publication or
release readiness for either repository.

## Disposition

The acceptance criterion of issue #1857 — `tokmd render --from-packets`
succeeds for all five Bun presets against producer output from this
repository — is proven on current main
(`779ded1715bbc323fd7275e8f393f97d6132138c`) with tokmd `1.15.0`. Generated
bundles and renders were kept under `/tmp` and are intentionally untracked.
Whether the per-packet versus top-level `preset_inputs` placement should be
reconciled so renders consume producer sections directly (instead of recording
manifest-level absence) is a producer/consumer contract question for maintainer
disposition, not a failure of this acceptance path.
