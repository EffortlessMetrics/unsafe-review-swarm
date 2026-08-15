# UNSAFE-REVIEW-SPEC-0044: issue-linked work specs

Status: proposed
Owner: repo-infra
Created: 2026-07-14
Linked proposal: UNSAFE-REVIEW-PROP-0002-source-of-truth-stack
Linked issues:
- https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1900
- https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1924
Support-tier impact: none
Policy impact:
- .allow/artifacts/doc-artifacts.toml
- docs/schemas/issue-work-spec.schema.json
- docs/schemas/bounded-subagent-brief.schema.json

## Problem

GitHub issues hold the operational portfolio, but a bounded issue/PR contract
needs a stable machine-readable shape that a controller, builder, reviewer,
verifier, and closeout can share without copying the whole issue queue into the
repository.

## Contract

Each work spec describes one issue-linked delivery unit. The version-one TOML
shape is defined by
`docs/schemas/issue-work-spec.schema.json` and exemplified by
`plans/work-specs/examples/UNSAFE-REVIEW-WORK-1900.toml`.

The required fields are the issue URL, work kind, objective, user outcome,
claim boundary, included/excluded scope, invariants, acceptance criteria with
proof commands, integration expectations, risks with mitigations, and rollback
strategy. Optional fields name dependencies/blockers, affected files/symbols,
linked durable specs/ADRs, compatibility posture, and later PR/closeout links.

Invariant and acceptance IDs are stable contract identifiers. Verification may
report evidence against them, but must not rewrite their meaning.

### Bounded delegation reference

`bounded-subagent-brief-v1` is the versioned, offline-validated input for one
bounded child action. Its JSON Schema shape lives at
`docs/schemas/bounded-subagent-brief.schema.json`, with TOML examples and
invalid fixtures under `plans/subagent-briefs/`.

The brief references the controlling issue and accepted work spec; it does not
copy or replace that contract. Every brief names a full base SHA, one action,
read and write scopes, durable authorities, proof obligations, non-goals, stop
conditions, and the expected `bounded-subagent-result-v1` return identifier.
The closed `capability` field is `write` only for `build` and `read_only` for
every other action. For read-only briefs, the checker also rejects a closed,
normalized vocabulary of mutation operations in `objective`,
`proof_obligations`, and `stop_when`. Normalization splits punctuation,
underscore, and CamelCase boundaries before matching, so prose in any
directive-bearing field cannot expand the declared capability.

Every `write_scope` entry is a canonical slash-separated repository-relative
path. Components use ASCII letters, digits, dot, underscore, or hyphen; the
first character of each component is not a dot. Repository roots (`.` or `/`),
absolute or drive-qualified paths, backslashes, `..` traversal, empty
components, and wildcard patterns are rejected. Authorities use one typed
form: `spec:ID`, `adr:ID`, `policy:path`, `work_spec:path`, `artifact:path`,
`issue:https://github.com/OWNER/REPO/issues/N`,
`pr:https://github.com/OWNER/REPO/pull/N`, or
`external:https://...`. Path-bearing authority payloads name files, never
directory-shaped trailing-slash paths, and preserve the canonical case of
tracked repository files. JSON Schema rejects exact duplicates; the
offline checker additionally compares normalized authority strings so casing
cannot disguise a duplicate. An external authority requires at least one
non-whitespace resource character after `https://`. Leading/trailing
whitespace, unknown types, duplicate
normalized references, and global/runtime authority are rejected.
Spec and ADR identifiers resolve to exactly one canonical tracked document.
GitHub issue and PR identities use closed owner, repository, route, and
canonical positive-decimal number grammars: the first digit is `1` through `9`
and remaining characters are ASCII digits, with no sign or leading zero.
Repository segments cannot be the URI dot segments `.` or `..`, and cannot end
with `.`.

Only `build` carries writer admission, an admitted worktree, and a non-empty
write-scope edit cage. `investigate`, `challenge_plan`, `verify`, `review`,
`triage_ci`, and `audit_cleanup` are read-only. `verify` and `review` additionally
require an exact PR and head SHA.

The checker resolves `work_item.work_spec` as one direct canonical TOML file
beneath `plans/work-specs/examples/` (nested paths are rejected), parses it through the accepted work-spec data,
and requires its `issue` URL to equal `work_item.issue`. Missing and mismatched
references fail offline.

`xtask check-subagent-briefs` validates the schema, one example for every
action, and rejected fixtures. Its single-line JSON result is structural
evidence only. The referenced result identifier does not implement or validate
the separate result contract tracked by issue #1926.

## Authority boundary

The work spec MUST NOT contain repository-wide priority, queue rank, current
task, default goal, or issue-status scheduling fields. GitHub issues and project
metadata own the concurrent work portfolio; the Codex controller's current
issue and phase remain ephemeral.

The bounded brief inherits that boundary and also rejects global-lane,
model-selection, agent-count, and private-reasoning authority. A brief cannot
admit a writer by itself: it records an admission decision already made from
live issue, branch, and worktree evidence.

Validation is offline and structural. It does not query GitHub, execute proof
commands, run tests, establish hosted-CI status, or make unsafe-code safety,
UB-free, Miri-clean, precision, recall, or merge-verdict claims.

Cargo-allow 0.1.10 has a closed artifact-kind vocabulary and no `work_spec`
kind. Until a compatible cargo-allow release adds one, the example is
registered as a draft `plan_item` for source-tree graph visibility while
`xtask check-work-specs` validates the content schema. This compatibility slot
must not be mistaken for a scheduler or for first-class cargo-allow support.

## Required evidence

```text
cargo run --locked -p xtask -- check-work-specs
cargo run --locked -p xtask -- check-subagent-briefs
cargo test --locked -p xtask work_specs
cargo test --locked -p xtask subagent_briefs
cargo-allow check --profile spec-system --mode audit
cargo run --locked -p xtask -- check-doc-artifacts
cargo run --locked -p xtask -- check-pr
git diff --check
```

## Follow-up boundary

Issue/link validation, surface vocabulary, PR/closeout lifecycle rules, and
controller/agent consumption are follow-up slices of #1900 and must not be
implemented by weakening this structural contract.
