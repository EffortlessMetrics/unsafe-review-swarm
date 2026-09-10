# Release documentation and support matrix

Audited 2026-09-09 against swarm `origin/main` at
`46afd7086a90a957f1fcdf08235e42721540253c` and the public source-of-record
release `v0.3.8` (`2026-06-18`). This is a documentation inventory, not a
candidate qualification or publication decision. The experimental/unavailable
posture below is unchanged; every surface row was re-checked and still holds
(MSRV floor is now 1.98, parser 0.0.350, all link targets present, schema
identities unchanged).

The [dependency ledger](DEPENDENCY_FREEZE.md) remains an inactive draft,
refreshed separately to the `b2d7bbff` snapshot. This audit does not activate
a freeze or rerun its receipts.

## Availability vocabulary

| Label | Meaning |
| --- | --- |
| Swarm main | Present in the unpublished workbench checkout; not a public release claim. |
| Public v0.3.8 | Available from the last published source/release surface. |
| Unavailable | Do not instruct users to acquire or depend on it. |
| Deferred | Intentionally outside the current documented surface. |

The next candidate has no frozen version; the draft cutline names candidate
`46afd708` (refresh PR #2190, still draft). The
[draft cutline](RELEASE_CUTLINE.md) and dependency ledger retain their own
audited inputs; neither is repinned or frozen by this documentation snapshot.

## Stale-claim matrix

| Surface | User-facing source | Current posture | Availability | Proof / limits |
| --- | --- | --- | --- | --- |
| First-use CLI | [`FIRST_USE.md`](../FIRST_USE.md), [`FIRST_HOUR.md`](../FIRST_HOUR.md), [`README.md`](../../README.md) | Experimental, advisory | Top-level preview-only `init`, the bounded action-first `pr` front panel, `doctor`, `explain`, `context`, and saved artifacts are on unpublished Swarm main; public v0.3.8 has no top-level `init` | `check-pr` and CLI e2e; `first-pr` remains the detailed compatibility route; no witness execution, source edits, automatic comments, blocking policy, or safety claim |
| Review bundle | [`FIRST_USE.md`](../FIRST_USE.md), [`PR_CI.md`](../ci/PR_CI.md) | Experimental, fixture/workflow-backed | Current bundle is on unpublished Swarm main; public v0.3.8 remains the last released artifact path | `check-first-pr-artifacts` and surface parity; tokmd rendering is not claimed by the producer alone |
| Saved LSP / agent packet | [`saved-lsp-json.md`](../editor/saved-lsp-json.md), [`agent-repair-workflow.md`](../explanation/agent-repair-workflow.md) | Experimental, read-only | Swarm main; no live-server or published-editor availability claim | Canonical ReviewCard projection; no source edits, witness execution, or repair success |
| VS Code / Open VSX | [`editor-extension.md`](../deferred/editor-extension.md), [`extension-mvp.md`](../editor/extension-mvp.md) | Experimental saved-bundle MVP; live client deferred | Swarm main packaging only; marketplace listings unavailable | Packaging and extension smoke lanes; no Marketplace/Open VSX publication |
| GitHub Action | [`github-action.md`](../ci/github-action.md) | Advisory integration surface | Candidate/source availability must be checked separately; public `v1` is unavailable | No automatic comments or default blocking; do not imply `@v1` resolves |
| Tokmd packets | [`tokmd-bun-packet-presets.md`](../dogfood/tokmd-bun-packet-presets.md), [#1857 receipt](../handoffs/2026-08-08-tokmd-packets-1857-acceptance.md) | Experimental producer/consumer contract | Historical five-preset receipt for `tokmd` `1.15.0` at `3d278c56d4afe37583e67500fc2e89e60c3077fe` and its named producer | The recorded integration test and preset executions do not qualify current candidate output; #1921 retains that execution gap |
| Ub-review handoff | [`PR_CI.md`](../ci/PR_CI.md), [#1890 receipt](../dogfood/reports/2026-08-08-cargo-allow-current-main.md) | Advisory packet/evidence route | Swarm main; publication and automatic comment posting unavailable | Packet integrity and dogfood checks; no witness, UB-free, or calibrated accuracy claim |
| Schema / compatibility | [`RELEASE_COMPATIBILITY.md`](RELEASE_COMPATIBILITY.md), [SPEC-0011](../specs/UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md), [`CHANGELOG.md`](../../CHANGELOG.md) | Accepted producer/verifier identity, required/additive-field, closed-vocabulary, producer-floor, and deprecation contract | Review-kit remains `0.1`; saved LSP is `0.2` and the first-PR verifier rejects `0.1`; legacy editor rendering is separately bounded by SPEC-0012 | #1921 must execute actual candidate output against named consumers; contract acceptance does not qualify them |
| Support tiers | [`SUPPORT_SUMMARY.md`](SUPPORT_SUMMARY.md), [`SUPPORT_TIERS.md`](SUPPORT_TIERS.md) | Experimental; no calibrated surface | Applies to the named evidence only | No current surface is a blocking policy, safety proof, UB-free claim, or calibrated precision/recall result |

## First-use contract

The public `v0.3.8` installation path remains:

```text
install the public v0.3.8 CLI
→ doctor
→ pr
→ open the public release's reviewer summary
→ explain/context or route to human review
→ run a named verification command externally
→ attach a receipt only when the external evidence matches the ReviewCard
```

For an explicitly identified unpublished swarm checkout, optional top-level
`init` previews repository adoption before `doctor` and `pr`; the integrated
`pr` route opens the bounded action-first front panel. `init` applies no
workflow or configuration, and explicit `--out` writes only the proposal JSON
in the selected directory. It is separate from `baseline init` and is absent
from the public `v0.3.8` parser linked in the
[compatibility receipt](RELEASE_COMPATIBILITY.md#cli-and-distribution-posture).
Installed candidate commands still require #1921 qualification. Editor,
Action, marketplace, and publication surfaces retain their experimental or
unavailable wording until their receipts independently prove otherwise.

## Claim boundary

This matrix says where documentation points and what evidence supports those
words. It does not prove analyzer accuracy, memory safety, UB-free status,
Miri cleanliness, site execution, publication authorization, or support on an
untested platform or consumer.
