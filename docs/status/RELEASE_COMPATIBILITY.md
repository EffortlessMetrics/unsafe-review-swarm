# Release compatibility receipt

Audited 2026-09-07 against swarm `origin/main` at
`a78770cfabe5daf631e5829133a0cae8db0210d7` and the last public release
`v0.3.8` (2026-06-18). The next candidate has no frozen version or candidate
SHA. This is a documentation inventory, not a compatibility guarantee,
installed qualification result, or publication decision.

The [dependency ledger](DEPENDENCY_FREEZE.md) remains a historical, inactive
draft at `125de5f683286c4e8da04b76c6633a2a8e123f5a`. This audit does not refresh
that ledger, activate a freeze, or rerun its receipts.

## Compatibility decision boundary

[SPEC-0011](../specs/UNSAFE-REVIEW-SPEC-0011-pr-ci-output.md)
defines the accepted producer/verifier contract for the eighteen exact
`(path, kind, format, schema_version/null)` artifact identities and their payload
discriminators. No bundle-wide version replaces those identities. Required
fields remain required; unknown object fields are additive and accepted unless
a field's contract explicitly closes its vocabulary. A non-empty additive
`operation_family` must be propagated consistently across the bundle. The
literal `unknown` and existing closed vocabularies retain their specified
meanings.

`tool_version` must be valid semver at least `0.3.8`; that producer floor does
not replace exact artifact identity checks. No field is deprecated by this
contract. Future renames or removals must follow SPEC-0011's documented
deprecation window of at least one published minor release, affected-artifact
schema change, and producer-floor advancement.

## Current serialized surfaces

| Surface | Current schema/evidence | Compatibility posture | Availability and limits |
| --- | --- | --- | --- |
| Analyze JSON | `0.1` plain output; `0.2` adds provenance and retains the `0.1` fields | `schema_version` is the route key; the additive relationship is documented in SPEC-0011 | Experimental ReviewCard projection; no broad published compatibility claim |
| Review-kit manifest | `0.1` | The verifier requires the eighteen exact artifact identities and matching payload discriminators in SPEC-0011 | Experimental first-PR artifact; no bundle schema bump is implied |
| Saved LSP | `0.2`; the first-PR verifier rejects `0.1` | [SPEC-0012](../specs/UNSAFE-REVIEW-SPEC-0012-lsp-editor-projection.md) separately permits legacy `0.1` action rendering; clients must not mix shapes or reconstruct executable `0.2` semantics from titles/arguments | Experimental/partial-runtime; no published editor integration claim |
| Gate manifest | `unsafe-review-gate/v1` | Envelope and dialect route consumers; advisory status is not a merge verdict | Swarm integration source only; public Action `v1` is unavailable |
| Repo scan status | `repo-scan-status/v1` | Status and partial artifacts must be treated as non-complete when capped, timed out, or failed | Experimental diagnosability contract; not coverage or safety proof |
| Manual candidates | `manual-candidate/v1`, `manual-candidates/v1` | Manual provenance remains separate from analyzer ReviewCards | Experimental, copy-only, not analyzer output or policy input |
| Tokmd packet input | `tokmd-packets/v1` | The historical [#1857 receipt](../handoffs/2026-08-08-tokmd-packets-1857-acceptance.md) covers five presets with `tokmd` `1.15.0` at `3d278c56d4afe37583e67500fc2e89e60c3077fe` | Experimental; that named producer/consumer pair does not qualify current candidate output |
| Repair queue | `0.1` | ReviewCard repair queue remains separate from manual-candidate repair handoff | Experimental, advisory, no automatic repair or agent execution |
| SARIF | Standard SARIF envelope with ReviewCard-derived result fields | Consumers must treat it as advisory static analysis output | Experimental code-scanning-compatible artifact; no policy or safety claim |

The accepted producer/verifier contract does not establish execution by Action,
tokmd, ub-review, saved-LSP/VS Code, SARIF, agent-packet, or repair-queue
consumers. Current-candidate execution remains unproven until #1921 records
the named consumer versions or commits and their results. Historical receipts
retain their original producer and consumer identities.

## CLI and distribution posture

- On unpublished swarm main, `pr` is the preferred first-use entrypoint and
  presents the bounded action-first front panel; `first-pr` and `review` remain
  compatibility names for the same advisory bundle and detailed route.
- `doctor`, `explain`, `context`, saved artifacts, and `baseline init` are
  present in the current tree. Top-level `init` is also integrated on
  unpublished swarm main: it previews an adoption proposal and applies no
  workflow or configuration. Explicit `--out` writes only the proposal JSON
  in the selected directory. Baseline creation remains a separate command.
- Public `v0.3.8` has `baseline init` but no top-level `init`, as recorded by
  its [command parser](https://github.com/EffortlessMetrics/unsafe-review/blob/9751f9567c21a64e830f2a64217fba04eb49b976/crates/unsafe-review-cli/src/parse.rs).
  Keep the public install path separate from the unpublished preview command.
- Workspace MSRV is Rust `1.95`; the three published packages remain `0.3.8`
  until a candidate version is explicitly frozen.
- The VS Code/Open VSX surface is a saved-bundle MVP. Marketplace listings,
  prebuilt binaries, crates.io candidate publication, GitHub Release, and
  public Action `v1` are unavailable until separately receipted.
- Live LSP exists as a read-only advisory server, while extension packaging
  remains partial-runtime. Neither implies editor marketplace availability.

## Required qualification handoff

Before #1921 PR2 can claim consumer compatibility, execute the matrix on one
exact candidate against the accepted contract and named consumer versions. The
receipt must name candidate/source/swarm SHAs, package versions, lockfile SHA,
environment/toolchain, command class, result, bounded failure classification,
and any skipped platform or consumer limitation. Full diagnostics stay in
protected execution logs; committed receipts contain only redacted summaries
and safe references or hashes.

## Claim boundary

This receipt documents current schemas, command posture, and availability
boundaries. It does not prove backward compatibility, analyzer accuracy,
memory safety, UB-free status, Miri cleanliness, site execution, broad
platform support, or publication authorization.
