# Spike 1928 — Project-scoped subagent, config, and skill surfaces

Status: advisory receipt — no production config added
Base: `d3f29236` (`main` after docs alignment, 2026-08-30)
Codex: `codex-cli 0.150.1` (`codex --version`)
Opencode: `1.18.23` (`opencode --version`)
Platform: `win32` (PowerShell 7+)

This receipt records which project-scoped subagent, config, and skill surfaces
are actually present or demonstrably supported on the current machine.
It does not prove that a prompt-level restriction is mechanically enforced.

## Scope and method

Read-only investigation of the worktree at `d3f29236` plus runtime help text
and one harmless read-only child execution. No `AGENTS.md` or `CLAUDE.md`
override was committed; no `.codex` production config was added.

Evidence commands (selection):

```powershell
gh issue view 1928 --json number,title,body
git rev-parse HEAD; git status --porcelain --branch
Get-ChildItem -Force -Recurse -Depth 3
codex --version; codex --help; codex exec --help; codex features list
Get-ChildItem $HOME/.codex -Force
Get-Content $HOME/.codex/config.toml
Get-ChildItem $HOME/.codex/skills -Recurse
opencode --version; opencode --help; opencode debug paths
opencode agent --help; opencode debug skill; opencode debug config
Get-ChildItem "<worktree>/.claude/agents"
Test-Path "<worktree>/.codex"
Test-Path "<worktree>/.opencode"
Test-Path "<worktree>/opencode.json*"
Select-String -Path "docs/contributing/*.md" -Pattern "claude|codex|opencode|agent|skill"
codex exec --skip-git-repo-check --json -C "F:/Temp/codex-probe-1928" "list files in current directory with ls"
```

One harmless Codex child was executed in an empty probe directory
(`F:/Temp/codex-probe-1928` with `test.txt`) via
`codex exec --skip-git-repo-check --json` and returned `test.txt`
successfully. One intentionally denied/mutation attempt, unknown-key
rejection, precedence, and recursion-depth verification were **not**
performed in this slice — see §4 gaps.

## 1. Claude Code — project-scoped surfaces (verified present)

| Surface | Location | Present | Verified | Evidence |
|---|---|---|---|---|
| Subagent definitions | `.claude/agents/` | yes | `Get-ChildItem .claude/agents` → 8 files | `<worktree>/.claude/agents:1` |
| — `artifact-verifier.md` | `.claude/agents/artifact-verifier.md` | yes | file read | list above |
| — `ci-log-triage.md` | `.claude/agents/ci-log-triage.md` | yes | file read | list above |
| — `claim-boundary.md` | `.claude/agents/claim-boundary.md` | yes | file read | list above |
| — `cleanup-auditor.md` | `.claude/agents/cleanup-auditor.md` | yes | file read | list above |
| — `implementer.md` | `.claude/agents/implementer.md` | yes | file read (frontmatter `name: implementer`, `tools: "*"`) | implementation packet read |
| — `issue-factcheck.md` | `.claude/agents/issue-factcheck.md` | yes | file read | list above |
| — `plan-refuter.md` | `.claude/agents/plan-refuter.md` | yes | file read | list above |
| — `repo-preflight.md` | `.claude/agents/repo-preflight.md` | yes | file read | list above |
| Runtime adapter | `CLAUDE.md` | yes | file read (maps Claude onto `AGENTS.md`) | `CLAUDE.md:1` |
| Root router / instructions | `AGENTS.md` | yes | file read | `AGENTS.md:1` |

`docs/contributing/LIFECYCLE_SURFACE_MAP.md` and
`docs/contributing/AGENT-ORCHESTRATION.md` document the lifecycle map and
explicitly state that `.claude/agents/*` are Claude-oriented runtime adapters,
not repository source-of-truth, and that model/tool pins are adapter-local.

No project-scoped Claude skills directory (e.g. `.claude/skills/`) was found
in the worktree; skill surfaces for this repo are not project-scoped files.
Global skill locations observed on this machine include
`~/.agents/skills` (≈17 skills: `api-contract-change`,
`changelog`, `ci-failure-triage`, `code-review`, `commit`, `config-env-change`,
`debug`, `dependency-change`, `docs-change`, `evidence-report`, `execute-spec`,
`fix`, `observability-change`, `pull-request`, `refactor`, `release-prep`,
`repo-discovery`, `schema-migration`, `security-sensitive-change`,
`test-authoring`, `upstream-issue`, etc.) and
`~/.claude/skills` — both surfaced via
`opencode debug skill`. These are **user-scoped**, not project-scoped.

## 2. Codex — project-scoped surfaces (observed)

| Surface | Location / flag | Present in repo | Runtime support (0.150.1) | Evidence |
|---|---|---|---|---|
| User config | `~/.codex/config.toml` (`$CODEX_HOME/config.toml`) | n/a (user-scoped) | yes — loaded by default; override via `-c key=value` | `codex --help` documents `-c/--config key=value` + `~/.codex/config.toml`; `Get-Content ~/.codex/config.toml` shows live file |
| Strict unknown-key rejection | `--strict-config` | n/a | yes — flag exists | `codex --help` and `codex exec --help` list `--strict-config` |
| Profile layering | `$CODEX_HOME/<name>.config.toml` | n/a | yes — `-p/--profile` | `codex --help: -p, --profile` |
| Project trust / per-path config | `~/.codex/config.toml` `[projects.'<path>'] trust_level = "trusted"` | n/a | yes — observed in live `config.toml` | `Get-Content ~/.codex/config.toml` shows many `[projects.'…']` entries including trust for `unsafe-review-swarm` |
| Root + nested instructions | `AGENTS.md` (root; nested `AGENTS.md` per directory) | repo has root `AGENTS.md`; no nested file observed | inferred yes — `~/.codex/AGENTS.md` exists and is the documented Codex instruction surface; repo `AGENTS.md` is present and listed as authority in `LIFECYCLE_SURFACE_MAP.md` | `Get-ChildItem ~/.codex/AGENTS.md` present; `<worktree>/AGENTS.md:1` present |
| Sandbox selection | `--sandbox read-only|workspace-write|danger-full-access` | n/a | yes — flag documented | `codex --help: -s, --sandbox` |
| Skills | `~/.codex/skills/` | n/a | yes — directory exists with `.system`, `fix-git-identity`, `worktree-build-cache-cleanup` plus system skills under `skills/.system/` | `Get-ChildItem ~/.codex/skills -Recurse` |
| Hooks | `~/.codex/config.toml` `[hooks.state]` | n/a | yes — observed trusted hashes | `Get-Content config.toml` shows `[hooks.state]` |
| MCP servers / features | `~/.codex/config.toml` `[mcp_servers.*]`, `[features]` | n/a | yes | `config.toml` and `codex features list` |

**Not present in the repo worktree and not verified as project-scoped in this spike:**

- `.codex/` directory (absent: `Test-Path .codex` → false)
- `.codex/config.toml` or `.codex/AGENTS.md` project-local config
- Any per-role file (e.g. coordinator/worker/reviewer role files)

The help text for Codex 0.150.1 references only `~/.codex/config.toml`
(`$CODEX_HOME/config.toml`) and `$CODEX_HOME/<profile>.config.toml`.
No help text in this version advertises a repository-local
`.codex/config.toml` as a layered config source. Whether Codex loads a
project-local `.codex/config.toml` when present was **not** tested in this
slice; treat the claim from issue #1901 as **unverified** on this version
until a controlled project-local file probe with `--strict-config` is run.

## 3. Opencode — project-scoped surfaces (observed)

| Surface | Location | Present in repo | Runtime support | Evidence |
|---|---|---|---|---|
| Project config | `opencode.json` / `opencode.jsonc` at repo root | absent | format exists (schema `https://opencode.ai/config.json`) | `Test-Path opencode.json*` false; `~/.config/opencode/opencode.jsonc` shows `{"$schema":"https://opencode.ai/config.json"}` |
| Project plugin dir | `.opencode/` at repo root | absent | directory surface exists in tool (`opencode debug`) but no repo instance | `Test-Path .opencode` false |
| Agent definitions | `opencode.json` `agents` / CLI `opencode agent list` | no repo file; global agents only | yes — `opencode agent --help`, `opencode debug agent <name>` | `opencode agent --help` lists `create`, `list`; `opencode debug agent --help` exists |
| Skill surfaces | `opencode debug skill` | no repo-bundled skills; global skills listed | yes — global + user skills enumerated | `opencode debug skill` enumerates `~/.agents/skills` and `~/.claude/skills` |
| Resolved effective config | `opencode debug config` | n/a | yes — dumps merged config including per-agent permission matrices | `opencode debug config` output includes per-agent `permission` tables (primary, plan, explore, general) |
| Global paths | `opencode debug paths` | n/a | yes | shows `config: ~/.config/opencode`, `data: ~/.local/share/opencode`, `tmp: F:/Temp/opencode` |

No project-scoped subagent, skill, or `opencode.json(c)` file exists in
`d3f29236`. Opencode's project scoping, if any, would be via a repository-local
`opencode.json(c)` or `.opencode/` directory, neither of which is present.
Verification of precedence, unknown-key handling, and per-agent sandboxing for
Opencode was not performed in this slice.

## 4. What this spike does NOT establish

- Precedence among root `AGENTS.md`, nested `AGENTS.md`, project `config.toml`,
  skills, and child instructions — not tested.
- Unknown-key and invalid-role behavior (silent ignore vs. strict error) —
  not tested beyond noting that `--strict-config` exists for Codex.
- Direct-child spawning mechanics, whether a child can spawn further children,
  and whether recursion/depth can be disabled or bounded — not tested.
- Which filesystem / command / Git / GitHub restrictions are mechanically
  enforced versus prompt-advisory — not tested.
- Role-specific instruction loading and result-return shape (bounded brief
  `#1924` / result `#1926`) — not in scope for this filesystem surface check.
- Mutation-denial proof (mechanical vs. prompt compliance) — not performed.

Those items remain open for the full capability receipt described in
#1928 Acceptance. Raw overflow logs (full `codex --help`, `opencode debug
config`, `gh issue view` JSON) are not committed; they are referenced here
as the bounded overflow for a follow-up probe.

## 5. Explicit unsupported / ambiguous surfaces on this version

- Codex project-local `.codex/config.toml` — **ambiguous**: no file present,
  no help text advertises it; do not treat as effective policy.
- Codex per-role sandboxes (`sandbox_permissions` per role) — **unverified**:
  no role files exist; treat as prompt guidance until mechanical test.
- Codex bounded thread/depth / direct-child-only delegation — **unverified**:
  no evidence in help text on this version that child spawning is limited to
  direct children or that depth is configurable.
- Opencode project-scoped agents/skills/config — **absent in this repo**:
  supported in principle via `opencode.json(c)` / `.opencode/` but no instance
  exists to verify precedence or enforcement.
- Claude project-scoped skills — **absent**: only subagents under
  `.claude/agents/` are project-scoped; skills are user-scoped on this machine.

## 6. Reproduction

```powershell
git worktree add F:/code/Opencode/Rust/ur-lanes/spike-1928 -b spike/1928-verify-surfaces d3f29236
codex --version; opencode --version
Get-ChildItem "<worktree>/.claude/agents"
Test-Path "<worktree>/.codex"; Test-Path "<worktree>/.opencode"
opencode debug skill; opencode debug paths; codex features list
# harmless child:
New-Item -ItemType Directory -Path F:/Temp/codex-probe-1928 -Force
codex exec --skip-git-repo-check --json -C F:/Temp/codex-probe-1928 "list files in current directory with ls"
```

## Claim boundary

This document proves which subagent/config/skill files are project-scoped in
`d3f29236` and which runtime surfaces are documented for Codex 0.150.1 /
Opencode 1.18.23 via help text and live config on this host. It does not prove
mechanical enforcement of read-only boundaries, sandboxing, or child-spawning
policy. The follow-up receipt for #1928 must perform the live probes listed
in §4 to separate advisory text from enforced controls.
