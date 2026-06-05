## Swarm routing

- [ ] Analyzer / evidence / ReviewCard correctness
- [ ] Fixture / calibration / dogfood
- [ ] Projection surface: CLI / PR artifact / saved LSP / agent packet / badge
- [ ] CI / source-of-truth rail / repository hygiene
- [ ] Docs / specs / handoff / roadmap
- [ ] Source promotion prep or source-sync acknowledgement

Source repo impact:
- [ ] none
- [ ] future curated promotion candidate
- [ ] source-sync acknowledgement needed after source-side work
- [ ] release/public package surface work only

Linked rail:
- Spec:
- Plan item:
- Roadmap item:

## Summary

-

## Links

Proposal:
ADR:
Issue:

## Scope

-
- [ ] One behavior / one seam / one policy slice
- [ ] No unrelated cleanup
- [ ] Generated receipts updated if needed

## Non-goals

-

## Support-tier impact

- [ ] none
- [ ] updates `docs/status/SUPPORT_TIERS.md`
- [ ] claim remains at the existing support level

## Policy impact

- [ ] No new panic-family calls without a receipt
- [ ] No bare `#[allow(clippy::...)]`
- [ ] Any `#[expect(...)]` has a policy-backed reason
- [ ] Non-Rust/source exceptions are receipted through policy TOML or future cargo-allow integration
- [ ] Unsafe changes have unsafe-review evidence or follow-up

Policy area:

- [ ] none
- [ ] accuracy calibration
- [ ] package boundary
- [ ] CI lane
- [ ] Clippy/lint
- [ ] no-panic
- [ ] non-Rust files
- [ ] doc artifacts

## Analyzer / ReviewCard checklist

For analyzer behavior changes only:

- [ ] operation family named
- [ ] obligation named
- [ ] newly accepted evidence described
- [ ] evidence target identity described
- [ ] positive fixture added or not applicable
- [ ] negative / false-positive-control fixture added or not applicable
- [ ] stale evidence control added or not applicable
- [ ] wrong-target control added or not applicable
- [ ] comment-only behavior covered or not applicable
- [ ] fixture golden updated or not applicable
- [ ] calibration ledger updated or not applicable
- [ ] support-tier wording stays within current proof
- [ ] public wording reviewed for overclaim

## CI economics

- Estimated default PR LEM:
- New default PR lanes:
- New label/main/nightly lanes:
- Expensive runners:
- Cache behavior:
- Branch-protection impact:

## Validation

-
- [ ] Local `cargo run --locked -p xtask -- check-pr`
- [ ] Relevant targeted tests
- [ ] ripr/unsafe-review/source-exception artifacts checked if applicable

## Rollback

-

## Claim boundary

What this PR proves:

What this PR does not prove:

Follow-ups:

## Boundaries

- [ ] No unproven support-tier promotion
- [ ] No witness execution by default
- [ ] No automatic comments
- [ ] No source edits
- [ ] No blocking policy
- [ ] No safety / UB-free / Miri-clean / site-execution claim

## Disposition authority

- [ ] I am not closing, merging, parking, superseding, or otherwise materially
      mutating this PR due to Codex session state, agent cap, or because
      another PR is active.
- [ ] If out-of-lane but aligned, I left the PR open as deferred, draft,
      blocked, or parked and named the next lane or owner decision needed.
- [ ] If closing, the repository-level reason is duplicate, superseded,
      rejected, abandoned, or unrecoverable.
- [ ] If closing as superseded, I linked the merged replacement.
- [ ] If parking, I left the PR open unless the owner explicitly requested
      closure.
