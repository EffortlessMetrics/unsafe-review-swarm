# cargo-allow issue #541 reproduction on current main (issue #1890)

Status: current evidence receipt recorded locally; no GitHub issue state
changed, no cargo-allow source modified, no publication performed

This report reruns the full repo-scan surface from public issue
[unsafe-review#541](https://github.com/EffortlessMetrics/unsafe-review/issues/541)
on a pinned cargo-allow checkout with the current `unsafe-review-swarm` main
build and the latest published binary, and classifies every current-main
actionable card into the issue #1890 bucket taxonomy. It refreshes
[2026-08-08-cargo-allow-current-main.md](2026-08-08-cargo-allow-current-main.md)
with a newer cargo-allow pin and a published-0.3.8 comparison, and supersedes
the older two-command comparison in
[2026-07-12-cargo-allow-541-current-repro.md](2026-07-12-cargo-allow-541-current-repro.md).

## Trust boundary

This is read-only static unsafe contract review evidence for one pinned
external repository. It is not a memory-safety proof, not a claim that
cargo-allow is safe, not UB-free status, not Miri-clean status, not a witness
result (no witness was executed or imported), not site-execution proof, not a
calibrated accuracy or precision/recall measurement, and not a policy or
release decision. The classification distinguishes fixture/test scope from
production source for this one repository only; it does not claim global
false-positive freedom.

## Scope and versions

- Target repository: `EffortlessMetrics/cargo-allow` (clean read-only clone)
- Pinned cargo-allow commit: `b0fcbcbe7a7291ef6ebfe590aefea515899c26b8`
  (`feat(parity): add deterministic comparison kernel (#3435)`)
- Swarm source commit under test:
  `9285040513b80279b3f2570e3ab4ede37b3ffb27` (`origin/main` at run time)
- Run date: 2026-08-10

| Binary | Reported version | Source identity | Build profile |
|---|---|---|---|
| crates.io release | `unsafe-review 0.3.8` | `cargo install unsafe-review --locked` | release |
| current swarm main | `unsafe-review 0.3.8` | `9285040513b80279b3f2570e3ab4ede37b3ffb27` | debug |

Both binaries report version string `0.3.8`; the workspace version has not been
bumped since the 0.3.8 release, so the builds are distinguished by source
commit, not by the reported version. The current-main runtime figures below are
from a debug build and are not comparable to the published release binary.

## Commands and runtimes

The six-command reproduction matrix from issue #1890 ran against the pinned
checkout with checkout-local output under
`target/unsafe-review/reports/`. Note that `badges --out` takes a directory;
the issue's literal path `badges.json` is created as a directory containing
the two badge files.

| Command (current main) | Exit | Runtime |
|---|---:|---:|
| `doctor --root .` | 0 | 1.0 s |
| `repo --root . --format json --out target/unsafe-review/reports/repo.json` | 0 | 53.6 s (status sidecar `elapsed_ms` 57486) |
| `badges --root . --out target/unsafe-review/reports/badges.json` | 0 | 54.6 s |
| `policy report --root . --format json --out target/unsafe-review/reports/policy-report.json` | 0 | 60.6 s |
| `receipt validate --root .` | 0 | 0.003 s (`witness receipts: 0 valid`) |
| `receipt audit --root . --format json --out target/unsafe-review/reports/receipt-audit.json` | 0 | 64.0 s |
| `repo --root . --format json --out .../repo-0.3.8-published.json` (published 0.3.8) | 0 | 9.0 s |

`doctor` reported a healthy read-only posture (git yes, base ref yes, artifact
dir writable; miri/careful/loom/shuttle/kani not configured). The repo scan
completed (`phase: complete`, 1035 files discovered, 1035 scanned, 0
remaining, `stop_reason: none`).

## Counts: does #541 still reproduce?

| Source | Cards | Open actionable | Contract missing | Guard missing | Other classes |
|---|---:|---:|---:|---:|---|
| #541 report (unsafe-review 0.1.0, historical) | 50 | 50 | 30 | 3 | `requires_loom` 5, `miri_unsupported` 12 |
| Published 0.3.8, pinned `b0fcbcbe` | 13 | 13 | 12 | 1 | 0 |
| Current swarm main, pinned `b0fcbcbe` | 13 | 13 | 12 | 1 | 0 |

Published 0.3.8 and current main emit the identical 13 card IDs with identical
per-card fields (the JSON bytes differ only in run metadata). Badges report
`unsafe-review: 13` and `unsafe-review+: 13`.

**Answer: #541 does not reproduce on current main.** The historical 50-card
projection and its false-actionable classes are gone. The historical defect
sites still exist in the pinned source — `unsafe { load(ptr) }` inside a
`#[test]` string literal in `crates/allow-rust/src/safety_comments.rs`, the
safe `push_finding(...)` helper in
`crates/allow-rust/src/line_unsafe_findings.rs`, and `.into_requirements()` in
the `#[cfg(test)]` module of `crates/allow-policy/src/toml_requirements.rs` —
and none of them produce cards. Those classes are
`already_fixed_since_original_report`.

Cross-check against the 2026-08-08 report (pin `8d85d5b2`): the semantic card
set is unchanged across the newer pin — same 13 sites, same operation hashes
(`set-var-b31c4cebfdaf`, `read-byte-c9aef8a4d43d`,
`read-96b6e660e28d`), and byte-identical badge hashes. The `set_var` card moved
from `cli.rs:148` to `cli.rs:152` with cargo-allow source drift. Card ID
prefixes differ (`UR-workspace-` here versus `UR-cargo-allow-1890-` there)
because the ID prefix derives from the scan root directory name, which differs
between the two runs.

## Artifact hashes and sizes (current main, pinned checkout)

Generated bundles stay untracked in the disposable cargo-allow checkout; only
hashes and sizes are recorded here.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `repo.json` | 109594 | `30f117a0a1e99b6d608700ff5fccbc8f846cab8a5f37b293b32bf0a7a0662119` |
| `repo.json.status.json` | 1255 | `a9f68dc2d44ccbba4946ec1d14c34ff6f67c3a293b02dfe9ab1997928070e0fd` |
| `badges.json/unsafe-review.json` | 93 | `a4500b9e7c18c49ccc78dcba2713e2637dd84ada405065eaec38bcf35e0e89e8` |
| `badges.json/unsafe-review-plus.json` | 94 | `947a0b1e840a3e407da4bcb43cbe8d7c3bb5c04673ddcd637562b9f8faf69f9e` |
| `policy-report.json` | 11424 | `c2f7f0042853ca5196e073f1abb9e273aadb5a0661a145c1a6836f09aa7b5e1e` |
| `receipt-audit.json` | 1409 | `d6411e812b1e4a491e9c8d72fa45114eea1783093d95fdb8c4fe4cdc4ecff3da` |
| `repo-0.3.8-published.json` (published 0.3.8) | 109568 | `aa99c4e2e5f88792e04a0041d6b50381d616e406f55d6a69dba2bcd8137fa0f9e` |

Bounded badge example (`unsafe-review.json`):

```json
{"schemaVersion": 1, "label": "unsafe-review", "message": "13", "color": "orange"}
```

## Card classification

Every one of the 13 current-main actionable cards is classified into exactly
one primary bucket from the issue #1890 taxonomy. Bucket totals:
`real_production_gap` 1, `test_or_fixture_scope` 12, all other buckets 0. The
historical classes `string_or_comment_content`, `safe_same_named_method`,
`function_definition_or_type_syntax`, and `wrong_receiver_or_origin` no longer
occur and count as `already_fixed_since_original_report` at the class level.

| # | Card site (file:line:col) | Card ID suffix | Family / class | Primary bucket | Why |
|---|---|---|---|---|---|
| 1 | `crates/cargo-allow/src/cli.rs:152:9` | `...set-var-b31c4cebfdaf-unknown-c1` | `unsafe_fn_call` / `guard_missing` | `real_production_gap` | Genuine production evidence gap; see below |
| 2 | `fixtures/unsafe/src/lib.rs:1:1` | `...unsafe_declaration-read-byte-c9aef8a4d43d-unknown-c1` | `unsafe_declaration` / `contract_missing` | `test_or_fixture_scope` | Intentional unsafe fixture crate; see below |
| 3 | `fixtures/unsafe/src/lib.rs:2:5` | `...raw_pointer_read-read-96b6e660e28d-pointer_validity-c1` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | Same fixture crate |
| 4 | `tests/fixtures/structural-identity/container_same_name_sibling_modules/after.rs:3:9` | `...after-rs-access-operation-...-c1` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | cargo-allow structural-identity test input |
| 5 | same `after.rs:8:9` | `...-c2` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | Same test input |
| 6 | same `before.rs:3:9` | `...before-rs-access-...-c1` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | Same test input |
| 7 | same `before.rs:8:9` | `...-c2` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | Same test input |
| 8 | `tests/fixtures/structural-identity/function_move/after.rs:2:5` | `...after-rs-read-right-...-c1` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | Same test input |
| 9 | same `after.rs:6:5` | `...after-rs-read-left-...-c1` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | Same test input |
| 10 | `tests/fixtures/structural-identity/function_move/before.rs:2:5` | `...before-rs-read-left-...-c1` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | Same test input |
| 11 | same `before.rs:6:5` | `...before-rs-read-right-...-c1` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | Same test input |
| 12 | `tests/fixtures/structural-identity/module_move/after.rs:2:5` | `...after-rs-access-...-c1` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | Same test input |
| 13 | `tests/fixtures/structural-identity/module_move/before.rs:3:9` | `...before-rs-access-...-c1` | `raw_pointer_read` / `contract_missing` | `test_or_fixture_scope` | Same test input |

Full card IDs are derivable from the recorded `repo.json` hash; the ID prefix
pattern is `UR-workspace-<path-slug>-<owner>-<kind>-<family>-<op>-<hash>-<class>-cN`.

### Card 1 detail: `real_production_gap`

- Card ID:
  `UR-workspace-crates-cargo-allow-src-cli-rs-run-operation-unsafe_fn_call-set-var-b31c4cebfdaf-unknown-c1`
- Source range: `crates/cargo-allow/src/cli.rs:152:9`, owner `run`, operation
  `unsafe { std::env::set_var("CARGO_ALLOW_QUIET", "1"); }`.
- Verification against the pinned source: the unsafe block sits in production
  CLI entry code (`pub(crate) fn run()`), guarded only by a `// Safety:`
  comment explaining the single-threaded call context. The card correctly
  recognizes that comment (contract is not in the `missing` list) and reports
  `guard_missing` with "Missing visible local guard for inferred safety
  obligations" and "No witness receipt imported". `reach` notes 13 test files
  mention owner `run`; `next_action` asks for obligation-specific guard
  evidence; `proof_path` is `human_review_only` with an honest
  `human-deep-review` route.
- Judgment: under the product's evidence model a `SAFETY` comment is not a
  guard and no witness receipt exists, so this is a true production evidence
  gap, not a false actionable. Detection, classification, and routing are all
  correct; no analyzer defect and no follow-up issue.

### Cards 2-13 detail: `test_or_fixture_scope`

- Why wrongly actionable for production review: every site is an intentional
  unsafe snippet inside cargo-allow's own test inputs —
  `fixtures/unsafe/src/lib.rs` (a deliberate unsafe fixture crate) and
  `tests/fixtures/structural-identity/**` (before/after pairs used by
  cargo-allow's structural-identity tests). Detection of the `unsafe fn`
  declaration and `core::ptr::read` operations is technically correct; the
  cards are inventory of test/fixture material, not production contract debt.
  Secondary observation: the structural-identity before/after pairs produce
  byte-similar duplicate cards (shared operation hash `96b6e660e28d`), a
  grouping-noise trait of the same surfacing seam.
- Owning seam: repo-scan surfacing/scope. `unsafe-review repo` discovery has
  no production-vs-fixture/test distinction, so fixture cards flow
  undifferentiated into `repo.json` counts, both badges, and the policy report
  summary. This is correct-but-loud evidence and routes to
  surfacing/grouping, not detection weakening (per the issue #1890 follow-up
  rule).
- Focused fixture in this repo: none. The swarm `fixtures/` tree and the
  repo-mode e2e tests (`fixtures/raw_pointer_alignment`, etc.) do not include
  a repo-mode fixture that mixes production and `fixtures/`+`tests/` unsafe
  code to pin scoping or labeling behavior.
- Proposed narrow regression case: a repo-mode fixture with one production
  unsafe site under `src/` and one deliberate unsafe site under
  `tests/fixtures/`, asserting that repo JSON either labels the fixture-scope
  card (for example a scope field or separate count) or that badges and the
  policy report split production open gaps from fixture/test inventory,
  whichever surfacing contract the follow-up issue chooses.

## Triage

| Target | Card or family | Primary label | Evidence | Follow-up |
|---|---|---|---|---|
| `cargo-allow-b0fcbcbe` | `cli.rs:152` `set_var` `guard_missing` | `actionable` | Production unsafe call with only a Safety comment; contract recognized, guard and witness honestly missing | `none` |
| `cargo-allow-b0fcbcbe` | 12 fixture/test-scope cards (`fixtures/unsafe`, `tests/fixtures/structural-identity`) | `noise` | Correctly detected intentional test-fixture unsafe code inflates headline gap counts and badges | one surfacing follow-up issue (below) |

`cargo-allow-b0fcbcbe` is an ad-hoc read-only scan label for this receipt, not
a committed corpus target.

## Follow-up issue proposals (one seam each)

1. **Repo-scan surfacing: scope or de-rank fixture/test-path cards in repo
   JSON, badges, and policy report.** Justification: 12 of 13 cards on a real
   consumer repository are intentional fixture/test inventory, and the
   `unsafe-review+` badge counts them identically to production gaps. This is
   the same class the 2026-07-12 receipt flagged, still present on current
   main. Route: ranking/grouping/surfacing only; detection stays as-is.

No other follow-up issue is justified. The single production card is a true
gap; the historical false-actionable classes from #541 are fixed; no
`string_or_comment_content`, `safe_same_named_method`,
`function_definition_or_type_syntax`, `wrong_receiver_or_origin`,
`inherited_or_non_actionable`, or `unclear_needs_human` cards remain.

## Disposition input for #541

For the source-issue disposition (to be posted by the controller, not by this
receipt): #541's 50-card false-actionable report does not reproduce on the
pinned current checkout with either the published 0.3.8 binary or current
swarm main; the remaining projection is 13 cards, of which 12 are fixture/test
inventory (one surfacing follow-up proposed above) and 1 is a genuine
production guard/witness evidence gap in `cli.rs`. No cargo-allow files,
issues, or PRs were modified.

## Deviations from the issue text

- `badges --out` consumes a directory path, so the artifact is
  `badges.json/unsafe-review.json` + `badges.json/unsafe-review-plus.json`,
  not a single `badges.json` file. Exit code 0; no behavior change needed for
  this receipt.
- The current-main binary was a debug build (`cargo build --locked -p
  unsafe-review-cli`); its runtimes are recorded but not compared against the
  release binary.
- No files were added to `corpus.toml`; this was a one-shot pinned scan per
  the issue's checkout-local instructions.
