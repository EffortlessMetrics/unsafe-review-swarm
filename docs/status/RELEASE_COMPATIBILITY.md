# Release compatibility receipt

Audited 2026-08-08 against swarm `origin/main` at
`6871dbe00c96fd30f531c8d6bb1140ea7310b83f` and the last public release
`v0.3.8` (2026-06-18). The next candidate has no frozen version or candidate
SHA. This is a documentation/support receipt, not a compatibility guarantee
or installed qualification result.

## Compatibility decision boundary

The repository has several versioned serialized surfaces, but the owner has
not yet approved one cross-consumer rule for unknown or additive fields. Until
that decision is recorded in the #1918 compatibility contract, candidate
qualification must report the consumer-policy row as `blocked` or `not_applicable`;
it must not infer an ignore, preserve, reject, or report policy from a parser
implementation. Schema routing and additive changes below are observations of
the current tree, not a public compatibility promise.

## Current serialized surfaces

| Surface | Current schema/evidence | Compatibility posture | Availability and limits |
| --- | --- | --- | --- |
| Analyze JSON | `0.1` plain output; `0.2` adds provenance and retains the `0.1` fields | `schema_version` is the route key; the additive relationship is documented in SPEC-0011 | Experimental ReviewCard projection; no broad published compatibility claim |
| Review-kit manifest | `0.1` | Bundle consumers must route by manifest schema and verify the artifact set | Experimental first-PR artifact; verifier proof is required |
| Saved LSP | `0.2`; legacy `0.1` action shape remains a compatibility path under SPEC-0012 | Read-only projection; live server and saved projection share ReviewCard identity | Experimental/partial-runtime; no published editor integration claim |
| Gate manifest | `unsafe-review-gate/v1` | Envelope and dialect route consumers; advisory status is not a merge verdict | Swarm integration source only; public Action `v1` is unavailable |
| Repo scan status | `repo-scan-status/v1` | Status and partial artifacts must be treated as non-complete when capped, timed out, or failed | Experimental diagnosability contract; not coverage or safety proof |
| Manual candidates | `manual-candidate/v1`, `manual-candidates/v1` | Manual provenance remains separate from analyzer ReviewCards | Experimental, copy-only, not analyzer output or policy input |
| Tokmd packet input | `tokmd-packets/v1` | Five named current-main presets have a bounded producer/consumer receipt | Experimental; no broad published tokmd compatibility claim |
| Repair queue | `0.1` | ReviewCard repair queue remains separate from manual-candidate repair handoff | Experimental, advisory, no automatic repair or agent execution |
| SARIF | Standard SARIF envelope with ReviewCard-derived result fields | Consumers must treat it as advisory static analysis output | Experimental code-scanning-compatible artifact; no policy or safety claim |

Unknown-field behavior across Action, tokmd, ub-review, saved LSP, VS Code,
SARIF, agent packets, and repair queues remains an owner decision. The
qualification manifest names this as a prerequisite rather than silently
turning observed serde behavior into a contract.

## CLI and distribution posture

- `pr` is the preferred first-use entrypoint; `first-pr` and `review` remain
  compatibility names for the same advisory bundle.
- `doctor`, `explain`, `context`, saved artifacts, and `baseline init` are
  present in the current tree. A completed top-level guided `init` is not
  present and must not be documented as shipped.
- Workspace MSRV is Rust `1.95`; the three published packages remain `0.3.8`
  until a candidate version is explicitly frozen.
- The VS Code/Open VSX surface is a saved-bundle MVP. Marketplace listings,
  prebuilt binaries, crates.io candidate publication, GitHub Release, and
  public Action `v1` are unavailable until separately receipted.
- Live LSP exists as a read-only advisory server, while extension packaging
  remains partial-runtime. Neither implies editor marketplace availability.

## Required qualification handoff

Before #1921 PR2 can claim consumer compatibility, record the owner decision
for unknown/additive fields and execute the matrix on one exact candidate. The
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
