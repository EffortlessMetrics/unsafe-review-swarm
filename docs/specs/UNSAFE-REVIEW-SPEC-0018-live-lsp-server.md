# UNSAFE-REVIEW-SPEC-0018: Live LSP server

Status: proposed
Owner: editor/lsp
Created: 2026-05-20
Linked proposal: ../proposals/UNSAFE-REVIEW-PROP-0001-product-contract.md
Linked ADRs:
- ../adr/UNSAFE-REVIEW-ADR-0002-review-card-is-canonical.md
- ../adr/UNSAFE-REVIEW-ADR-0005-advisory-first-policy.md
- ../adr/UNSAFE-REVIEW-ADR-0006-live-lsp-server-is-read-only.md
Linked specs:
- UNSAFE-REVIEW-SPEC-0002-review-card-schema.md
- UNSAFE-REVIEW-SPEC-0003-input-scope-and-diff-model.md
- UNSAFE-REVIEW-SPEC-0012-lsp-editor-projection.md
- UNSAFE-REVIEW-SPEC-0013-agent-packets.md
Support-tier impact: yes
Policy impact: none initially
Linked plan:
- ../../plans/lsp-server/implementation-plan.md

## Problem

`unsafe-review` already emits a saved LSP/editor projection with diagnostics, hovers, status, and command-style action data. That projection is useful for adapters, but it is not a live LSP server. The repo needs a precise, enforceable contract for adding `unsafe-review lsp` with `tower-lsp-server` without weakening the existing trust boundary.

The live server must make ReviewCards visible at coding time while preserving the product posture:

```text
static unsafe-contract review evidence
not memory-safety proof
not UB-free status
not Miri-clean status
not witness execution
not policy authority
not source editing
```

## Decision boundary

The live LSP server is a **projection surface**, not a new analyzer.

It may:

- run unsafe-review-core analysis
- publish diagnostics derived from ReviewCards
- show ReviewCard hovers
- return command-only code actions
- return bounded packet / witness command payloads
- log advisory status
- refresh on save or explicit command

It must not:

- create analyzer truth outside ReviewCards
- edit source
- generate patches
- insert SAFETY comments
- run Miri, sanitizers, Loom, Shuttle, Kani, Crux, fuzzing, or mutation tests
- create witness receipts
- import witness receipts unless explicitly routed through existing receipt surfaces
- post PR comments
- decide policy
- enable blocking behavior by default
- claim a test executed an unsafe site without receipt evidence
- treat no diagnostics as proof of safety

## Implementation owner

The first live server belongs in `unsafe-review-cli`.

Rationale:

```text
unsafe-review          = install façade / product handle
unsafe-review-cli      = command parser, runtime adapter, LSP stdio server
unsafe-review-core     = analysis engine and canonical ReviewCard facts
```

`unsafe-review-core` remains free of `tower-lsp-server` and Tokio runtime obligations. `unsafe-review-cli` can depend on `tower-lsp-server` and Tokio because it owns runtime behavior.

## Dependency rail

Use:

```toml
tower-lsp-server = "0.23.0"
tokio = { version = "1", features = ["rt-multi-thread", "io-std", "io-util", "sync", "time"] }
```

The first implementation should use the default Tokio-backed mode. Do not add a runtime-agnostic feature path until a real downstream need exists.

## CLI contract

Add:

```bash
unsafe-review lsp
```

Initial CLI behavior:

- starts a stdio LSP server
- never prints non-LSP stdout
- logs through LSP `client/logMessage`
- exits 0 after normal shutdown
- exits 2 for startup/runtime setup errors before serving

`stderr` may contain pre-LSP startup errors only. After serving begins, diagnostics/status go through LSP.

## LSP capability contract

`initialize` must return:

- `textDocumentSync = FULL`
- `hoverProvider = true`
- `codeActionProvider = true`
- `executeCommandProvider.commands` includes:
  - `unsafe-review.refresh`
  - `unsafe-review.collectAgentPacket`
  - `unsafe-review.explainWitnessRoute`
  - `unsafe-review.collectWitnessCommand`
  - `unsafe-review.openRelatedTest`

Forbidden first-slice capabilities include completion, rename, formatting, documentSymbol, semanticTokens, inlayHint, codeLens, and workspace edits.

## Root resolution

The server resolves root in this order:

1. first workspace folder URI
2. rootUri
3. current working directory

If root cannot be converted to a local path, use current working directory, log a warning, and do not fail initialization.

## Configuration contract

Initialization options may include:

```json
{
  "unsafeReview": {
    "mode": "repo",
    "base": "origin/main",
    "maxCards": 100,
    "refreshOnInitialize": true,
    "refreshOnOpen": false,
    "refreshOnSave": true
  }
}
```

Defaults:

- `mode = repo`
- `base = null`
- `maxCards = null`
- `refreshOnInitialize = true`
- `refreshOnOpen = false`
- `refreshOnSave = true`
- `policy = advisory`, always

Invalid configuration must log warning, fall back to defaults, and must not enable blocking or no-new-debt policy.

## Refresh and safety rails

- Saved-files server only in v1.
- `didChange` must not analyze unsaved buffer content by default.
- Single in-flight refresh guard.
- `git diff` failures must be logged and must not silently become a clean repo scan.
- Analysis and `spawn_blocking` failures must be logged.
- Refresh failures must not imply clean/safe state.
- Refresh failures clear stale diagnostics or mark status stale.
- Stale generations must not publish diagnostics.
- Refresh publishing must not hold state locks across `.await`.
- `AnalyzeOutput`/`ReviewCard` remain canonical facts.

## Diagnostics / hover / actions

- One `ReviewCard` maps to one `Diagnostic`.
- High priority maps to Warning; all others to Information.
- No `Error` severity in v1.
- Diagnostic ranges use UTF-16 character width.
- Diagnostic `data` includes `card_id`, `operation_family`, `hazards`, `missing_evidence`, and trust boundary metadata.
- Hover is derived from the `ReviewCard` under the URI and cursor position and
  includes obligations, evidence summary, missing evidence, next action,
  optional witness route, and trust boundary.
- Hover must not overclaim safety/soundness/UB-free/Miri-clean status.
- All code actions are card-scoped and command-only (`edit == None`).

## Execute command contract

Supported commands:

- `unsafe-review.refresh`
- `unsafe-review.collectAgentPacket`
- `unsafe-review.explainWitnessRoute`
- `unsafe-review.collectWitnessCommand`
- `unsafe-review.openRelatedTest`

Each command returns bounded payloads (or `null`) and must not edit source or execute witness tools.
Command arguments use stable object payloads that include `card_id` when a
command is card-scoped.

## Security and policy contract

The server is local, read-only, and advisory:

- no network/provider/model calls
- no source writes
- no receipt writes
- no policy writes
- no comment posting
- no witness execution

If diff mode shells out, command must be fixed `git diff <base>...HEAD` with argument-safe process invocation from configured root.

## Module layout target

```text
crates/unsafe-review-cli/src/lsp.rs
crates/unsafe-review-cli/src/lsp/
  backend.rs
  capabilities.rs
  config.rs
  diagnostics.rs
  hover.rs
  actions.rs
  state.rs
  uri.rs
  tests.rs
```

## CI proof

```bash
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test -p unsafe-review-core lsp_projection --locked
cargo test -p unsafe-review-cli lsp --locked
cargo run --locked -p xtask -- lsp-smoke
cargo run --locked -p xtask -- check-pr
git diff --check
```
