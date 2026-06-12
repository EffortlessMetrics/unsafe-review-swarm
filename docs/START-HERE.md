# Start here: adopting unsafe-review

`unsafe-review` is an advisory unsafe-evidence layer. It finds unsafe Rust
changes that are missing a safety contract, local guard, test reach, or witness
receipt. It is not a UB oracle and not a memory-safety proof. Every output
surface preserves that boundary: no memory-safety proof, no UB-free claim, no
Miri-clean claim, no site-execution claim, no calibrated precision/recall claim,
no default comment posting, and no default blocking policy. `ub-review` is the
LLM orchestrator that owns posting and blocking decisions; `unsafe-review`
supplies the coverage artifacts those layers consume.

The authoritative surface and boundary map is
[UNSAFE-REVIEW-SPEC-0028](specs/UNSAFE-REVIEW-SPEC-0028-delivery-surfaces-and-ease-of-use.md).

---

## Quickstart

```bash
cargo install unsafe-review --locked
unsafe-review pr
```

`unsafe-review pr` is the zero-config entry point (alias for `first-pr` with
auto-detected base). It produces an advisory PR bundle under
`target/unsafe-review/`: review cards, a PR summary, SARIF, a comment plan, an
LSP projection, and the ub-review gate manifest.

---

## The five delivery surfaces

| Surface | How you get it | Key artifact | Owning spec(s) |
|---|---|---|---|
| **Repo badge** | `unsafe-review badges --out badges/` on main; serve via Shields endpoint | `badges/unsafe-review.json` | [SPEC-0014](specs/UNSAFE-REVIEW-SPEC-0014-repo-inventory-badges.md), [SPEC-0031](specs/UNSAFE-REVIEW-SPEC-0031-baseline-aware-badge.md) |
| **PR gate / GitHub Action** | `uses: EffortlessMetrics/unsafe-review@v1` in workflow | `bundle_dir` and `gate_status` step outputs; bundle contains `unsafe-review-gate.json` | [SPEC-0037](specs/UNSAFE-REVIEW-SPEC-0037-pr-gate-composite-action.md), [docs/ci/github-action.md](ci/github-action.md) |
| **PR line comments / comment plan** | `unsafe-review pr` produces `comment-plan.json` — a bounded comment plan. `unsafe-review` does not post; a downstream consumer (ub-review when embedded, or a gate-workflow trusted-poster) reads the plan and posts. | `comment-plan.json` | [SPEC-0022](specs/UNSAFE-REVIEW-SPEC-0022-pr-commenting-experience.md), [SPEC-0032](specs/UNSAFE-REVIEW-SPEC-0032-comment-plan-coverage-hardening.md) |
| **LSP / editor diagnostics and agent context** | `unsafe-review pr` emits `lsp.json`; `unsafe-review context <card-id> --json` or `--file F --lines Y-Z --json` for a per-card packet | `lsp.json`, agent packet JSON | [SPEC-0012](specs/UNSAFE-REVIEW-SPEC-0012-lsp-editor-projection.md), [SPEC-0013](specs/UNSAFE-REVIEW-SPEC-0013-agent-packets.md), [SPEC-0018](specs/UNSAFE-REVIEW-SPEC-0018-live-lsp-server.md), [SPEC-0033](specs/UNSAFE-REVIEW-SPEC-0033-llm-context-packet.md) |
| **ub-review integration** | `ub-review` reads `unsafe-review-gate.json` from the PR bundle | `unsafe-review-gate.json` | [SPEC-0034](specs/UNSAFE-REVIEW-SPEC-0034-ub-review-gate-manifest.md) |

---

## Is it cheap and low-noise?

See [SPEC-0038](specs/UNSAFE-REVIEW-SPEC-0038-low-noise-usefulness-telemetry.md)
for usefulness telemetry and
[SPEC-0039](specs/UNSAFE-REVIEW-SPEC-0039-scheduled-corpus-backstop.md) for the
scheduled corpus backstop that provides resource and timing signals.
