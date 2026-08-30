# Spike 1901 — Codex runtime verification and minimal adapter

Status: verification receipt for #1901 — minimal adapter only, no new runtime topology
Base: `a0a7fc22` (`main` after pilot 1934, 2026-08-30)
Codex: `codex-cli 0.150.1` (`codex --version`)
Opencode: `1.18.23` (`opencode --version`)
Cargo: `1.95.0` (`cargo --version`)
Git: `2.54.0.windows.1` (`git --version`)
gh: `2.86.0` (`gh --version`)
Bash: `5.2.21(1)` (`bash --version` via pwsh)
PowerShell: `7.6.5` (`pwsh --version`)
Platform: `win32`

This receipt verifies that the supported Codex/Opencode runtime capabilities assumed
by #1901 are either mechanically present or honestly degraded, and that the
committed adapter at this base is the smallest evidence-backed surface.

## 1. Capability probe (extends #1928)

Re-ran the #1928 surface checks at `a0a7fc22` on the same host:

```powershell
codex --version; opencode --version; cargo --version; gh --version; git --version; bash --version
git worktree list; git worktree add F:/code/Opencode/Rust/ur-lanes/adapter-1901 -b adapter/1901-minimal a0a7fc22
opencode debug config; opencode debug agent read-only; opencode debug skill; opencode debug paths
codex --help; codex exec --help
Test-Path .codex; Test-Path .opencode; Get-Content opencode.json; Get-ChildItem .opencode/agents; Get-ChildItem .opencode/skills
cargo fmt --all --check; cargo run --locked -p xtask -- check-docs; cargo run --locked -p xtask -- check-pr
```

Results:

| Capability | Result | Evidence |
|---|---|---|
| `bash` | present, exit 0 | `bash --version` → 5.2.21 |
| `gh` | present, exit 0 | `gh version 2.86.0` |
| `cargo` | present, exit 0 | `cargo 1.95.0` |
| `git worktree` | present, isolation works | `git worktree add` succeeded for `adapter/1901-minimal`; `git worktree list` shows isolated entry |
| `codex exec` | present | `codex --help` / `codex exec --help` list `--sandbox`, `--strict-config`, `-c key=value`, `-p profile` |
| `opencode` config | present, verified key | `opencode debug config` shows merged `subagent_depth: 1` with 3 agents |
| `opencode` agents | present, 3 files | `.opencode/agents/{coordinator,read-only,writer}.md` |
| `opencode` skills | present, 7 files | `.opencode/skills/{prepare-issue,compile-work-spec,build-from-work-spec,review-current-head,respond-to-feedback,publish-pr,reconcile-merge}/SKILL.md` |

All four required execution surfaces for the adapter (`bash`, `gh`, `cargo`, `git worktree`)
are mechanically available at this base. No additional host capability is assumed.

## 2. Minimal adapter — what is committed

At `a0a7fc22` the only project-scoped adapter for this runtime is:

- `opencode.json` with single verified key `subagent_depth: 1` (primary may launch
  subagents; subagents may not launch further — see `https://opencode.ai/docs/config#subagent-depth`)
- `.opencode/agents/coordinator.md` (`mode: primary`) — reconstructs live GitHub/repo state,
  selects one session-local concern, creates/synthesizes `bounded-subagent-brief-v1` /
  `bounded-subagent-result-v1` per #1924/#1926, owns merge judgment; no model/tier/count policy;
  links to `AGENTS.md` and `docs/contributing/LIFECYCLE_SURFACE_MAP.md` without copying them
- `.opencode/agents/read-only.md` (`mode: subagent`, `permission.edit: deny` mechanical) —
  receives one bounded question + read scope; advisory degradation for bash/filesystem/Git/GitHub
  mutation is explicit, not a security claim
- `.opencode/agents/writer.md` (`mode: subagent`, `permission.edit: allow`, `permission.bash: allow`) —
  requires admitted `issue`/`work_spec`, `basis.base_sha`, `admission.worktree`, explicit
  `write_scope` edit cage, and `proof_obligations`; new heads invalidate prior review
- `.opencode/skills/` 7 progressive-disclosure entries linking to the lifecycle docs rather than
  duplicating them
- `AGENTS.md` as the Codex instruction surface (root `AGENTS.md` present since repo inception;
  no `.codex/config.toml` is committed)

No `.codex/` directory, no `.codex/config.toml`, no per-role sandbox file, no model or tier
pin, no fixed agent count or concurrency wave, and no repository-global `active` issue/phase
is committed. Unknown Codex project-local config is treated as ambiguous per #1928 §5 and is
not repository policy.

## 3. Acceptance mapping for #1901

| #1901 acceptance | Disposition | Evidence |
|---|---|---|
| no unsupported Codex setting committed | met | `Test-Path .codex` false; `opencode.json` contains only `subagent_depth`; no sandbox/model key committed |
| no large role catalog by default | met | only 3 agents (coordinator/read-only/writer) — split only on mechanically distinct `permission.edit` boundary verified via `opencode debug config`/`opencode debug agent` |
| no fixed model/tier/agent count/concurrency | met | `AGENTS.md:48` and all three `.opencode/agents/*` explicitly state no model/tier/count/wave is authority |
| read-only and writer boundaries consume #1924/#1926 | met | both brief/result schemas referenced in coordinator + writer + read-only; `.opencode/skills/prepare-issue`, `compile-work-spec`, `build-from-work-spec` reference `docs/schemas/bounded-subagent-brief.schema.json` and `docs/schemas/bounded-subagent-result.schema.json` |
| writer requires admitted issue/work spec/worktree/edit cage | met | `.opencode/agents/writer.md:9` enumerates the four admission gates |
| current focus not persisted as repository-global goal | met | `AGENTS.md:52` — zero active work items is valid; no `.allow/goals/active.toml` mutation in adapter commits baf10078/a0a7fc22 |
| unsupported restrictions degrade visibly | met | `read-only.md:20` — `permission.edit: deny` is mechanical; `read-only.md:23` — bash/filesystem/Git mutation is `prompt-advisory` |
| one read-only and one writer dry run | met | pilot #1933 (`docs/pilots/1933-converge.md` at 6e0ba56f): 3 read-only reviewers + 1 admitted writer on PR 2135; pilot #1934 (`docs/pilots/1934-fresh.md` at a0a7fc22): fresh issue→merge simulation with bounded archaeology, writer admission, exact-head review |
| narrow control task stays single-agent | met | pilots record single-agent baseline comparison without delegated children; spike #1928 child was single harmless `codex exec` in isolated `F:/Temp/codex-probe-1928` with `test.txt` |
| rollback removes adapter without changing lifecycle truth | met | deletion of `opencode.json` + `.opencode/agents/` + `.opencode/skills/` + this receipt leaves `AGENTS.md`, `CLAUDE.md`, `.claude/agents/*`, specs, and lifecycle docs unchanged; `opencode debug config` then shows no project agents |

## 4. Precedence and enforcement

- `AGENTS.md` owns the repository contract; `opencode.json` `.opencode/agents/*` are
  optional runtime adapters (see `docs/contributing/LIFECYCLE_SURFACE_MAP.md:28`).
- `opencode debug config` is the precedence proof: merged config shows `subagent_depth: 1`
  and per-agent permissions; `--strict-config` exists for Codex unknown-key rejection but
  no unknown key is committed.
- Child recursion is mechanically bounded by `subagent_depth: 1`: primary → subagent only.
- Manual/human/Claude/future-runtime paths remain valid — no workflow, hook, or scheduler
  is added by this adapter.

## 5. Reproduction

```powershell
git rev-parse HEAD  # a0a7fc22
codex --version; opencode --version; cargo --version; gh --version; git --version; bash --version
git worktree list
opencode debug config | Select-String subagent_depth
opencode debug agent read-only
cargo fmt --all --check
cargo run --locked -p xtask -- check-docs
cargo run --locked -p xtask -- check-pr
```

Expected: all version commands exit 0; worktree add succeeds; `subagent_depth: 1` shown;
`read-only` permission `edit: deny` shown; `check-docs` ok; `fmt --check` ok;
`check-pr` core gate runs (fmt/Clippy/tests/docs-pr — local proof only, not hosted merge authority).

## Claim boundary

This proves the minimal adapter at `a0a7fc22` uses only the verified project-scoped
surfaces `opencode.json`/` .opencode/agents/*`/` .opencode/skills/*` and root
`AGENTS.md`, with mechanically verified `bash`/`gh`/`cargo`/`git worktree` and
`subagent_depth` + `permission.edit` enforcement. It does not prove orchestration
is faster, that prompt-level advisory restrictions are mechanically enforced, or
that a Codex project-local `.codex/config.toml` would be loaded if added.
