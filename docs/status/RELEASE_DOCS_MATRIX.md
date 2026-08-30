# Release documentation and support matrix

Audited 2026-08-30 against swarm `origin/main` at
`c741f69d75e9fc9f590b638d37086b6e87d65e68` (dependency freeze
`125de5f683286c4e8da04b76c6633a2a8e123f5a`) and the public source-of-record
release `v0.3.8` (`2026-06-18`). This is an inventory and claim-boundary
receipt, not a candidate qualification or publication decision. Recent
integrations through `c741f69d` include rowless ReviewCard detail inventories
(PRs #2124–#2127: shared detail fields, agent packet, witness plan, outcome
snapshot parity), first-PR terminal ReviewCard parity (#2121), baseline-add
ReviewCard snapshot parity (#2122), and `xtask` check-dispatch extraction
(#1806 slice at `ed345b71`); these are present on swarm main as observed
inventory and do not change the experimental/unavailable posture below.

## Availability vocabulary

| Label | Meaning |
| --- | --- |
| Swarm main | Present in the unpublished workbench checkout; not a public release claim. |
| Public v0.3.8 | Available from the last published source/release surface. |
| Unavailable | Do not instruct users to acquire or depend on it. |
| Deferred | Intentionally outside the current documented surface. |

The next candidate has no frozen version or candidate SHA. The draft cutline
records the exact current-main snapshot and must be refreshed after any
candidate, source, or dependency mutation.

## Stale-claim matrix

| Surface | User-facing source | Current posture | Availability | Proof / limits |
| --- | --- | --- | --- | --- |
| First-use CLI | [`FIRST_USE.md`](../FIRST_USE.md), [`FIRST_HOUR.md`](../FIRST_HOUR.md), [`README.md`](../../README.md) | Experimental, advisory | Current bounded action-first `pr` front panel, `doctor`, `explain`, `context`, and saved artifacts are on unpublished Swarm main; public v0.3.8 remains the last released behavior | `check-pr` and CLI e2e; `first-pr` remains the detailed compatibility route; no witness execution, source edits, automatic comments, blocking policy, or safety claim |
| Review bundle | [`FIRST_USE.md`](../FIRST_USE.md), [`PR_CI.md`](../ci/PR_CI.md) | Experimental, fixture/workflow-backed | Current bundle is on unpublished Swarm main; public v0.3.8 remains the last released artifact path | `check-first-pr-artifacts` and surface parity; tokmd rendering is not claimed by the producer alone |
| Saved LSP / agent packet | [`saved-lsp-json.md`](../editor/saved-lsp-json.md), [`agent-repair-workflow.md`](../explanation/agent-repair-workflow.md) | Experimental, read-only | Swarm main; no live-server or published-editor availability claim | Canonical ReviewCard projection; no source edits, witness execution, or repair success |
| VS Code / Open VSX | [`editor-extension.md`](../deferred/editor-extension.md), [`extension-mvp.md`](../editor/extension-mvp.md) | Experimental saved-bundle MVP; live client deferred | Swarm main packaging only; marketplace listings unavailable | Packaging and extension smoke lanes; no Marketplace/Open VSX publication |
| GitHub Action | [`github-action.md`](../ci/github-action.md) | Advisory integration surface | Candidate/source availability must be checked separately; public `v1` is unavailable | No automatic comments or default blocking; do not imply `@v1` resolves |
| Tokmd packets | [`tokmd-bun-packet-presets.md`](../dogfood/tokmd-bun-packet-presets.md), [#1857 receipt](../handoffs/2026-08-08-tokmd-packets-1857-acceptance.md) | Experimental producer/consumer contract | Five named presets validated for the current-main consumer only | `cargo test --locked -p tokmd --test render_packets_integration`; no broad published compatibility claim |
| Ub-review handoff | [`PR_CI.md`](../ci/PR_CI.md), [#1890 receipt](../dogfood/reports/2026-08-08-cargo-allow-current-main.md) | Advisory packet/evidence route | Swarm main; publication and automatic comment posting unavailable | Packet integrity and dogfood checks; no witness, UB-free, or calibrated accuracy claim |
| Schema / compatibility | [`RELEASE_COMPATIBILITY.md`](RELEASE_COMPATIBILITY.md), [`CHANGELOG.md`](../../CHANGELOG.md), [`SUPPORT_TIERS.md`](SUPPORT_TIERS.md) | Versioned, additive where explicitly documented; unknown-field policy owner-pending | Current schemas are candidate inputs, not a frozen public compatibility promise | Consumers must route on schema/version; #1921 cannot claim cross-consumer compatibility until the owner decision is recorded |
| Support tiers | [`SUPPORT_SUMMARY.md`](SUPPORT_SUMMARY.md), [`SUPPORT_TIERS.md`](SUPPORT_TIERS.md) | Experimental; no calibrated surface | Applies to the named evidence only | No current surface is a blocking policy, safety proof, UB-free claim, or calibrated precision/recall result |

## First-use contract

The truthful documented path is:

```text
install the public v0.3.8 CLI (or identify the local candidate explicitly)
→ doctor
→ pr
→ open the bounded action-first reviewer front panel
→ explain/context or route to human review
→ run a named verification command externally
→ attach a receipt only when the external evidence matches the ReviewCard
```

`init` is not included in this path: the current release cutline contains a
partial preview-only baseline slice, not a completed top-level guided adoption
command. Likewise, editor, Action, marketplace, and publication surfaces must
retain their explicit experimental or unavailable wording until their receipts
independently prove otherwise.

## Claim boundary

This matrix says where documentation points and what evidence supports those
words. It does not prove analyzer accuracy, memory safety, UB-free status,
Miri cleanliness, site execution, publication authorization, or support on an
untested platform or consumer.
