# Repo style

This repo is operated as an evidence machine.

Rust and `xtask` are the default construction material. Non-Rust files,
unsafe, panic paths, lint suppressions, generated files, workflow behavior,
process/network access, expensive CI lanes, and release claims must be owned
and receipted.

Static evidence runs first:

- source-exception evidence through the repo policy ledgers and the future
  `cargo-allow` wrapper;
- static mutation-exposure evidence through the future `ripr` lane;
- unsafe-contract review evidence through `unsafe-review` ReviewCards;
- rustc and Clippy evidence for code-shape policy.

Runtime evidence runs where it pays:

- focused tests on PRs;
- targeted mutation for risk PRs;
- broader mutation, Miri, fuzz, and coverage lanes on main, nightly, release,
  or explicit full-validation paths.

CI is designed for proof per Linux-equivalent minute. Default PRs are cheap,
deterministic, and high-signal. Deep validation is preserved, but routed by
risk pack, label, main, nightly, release, or explicit owner action.

Agents work one review-fast PR at a time. Review-fast does not mean tiny; it
means coherent seam, nearby proof, efficient verification, and honest claim
boundary. Do not broaden scope to satisfy CI. Do not add invisible exceptions.

## Tool roles

`xtask` is the repo-facing control plane. It may wrap upstream tools, aggregate
receipts, and enforce repo-local glue, but it must not become a reimplementation
of every upstream engine.

| Tool or plane | Role | Claim boundary |
| --- | --- | --- |
| Policy ledgers and future `cargo-allow` integration | Durable ownership for source-tree exceptions | Exception ownership, not proof that the exception is correct |
| `ripr` | Static mutation-exposure analysis | Weak-oracle exposure signal, not killed/survived runtime mutation proof |
| `unsafe-review` | Advisory unsafe-contract reviewability | ReviewCard evidence, not memory-safety or UB-free proof |
| `cargo-mutants` | Runtime mutation backstop | Runtime mutation outcomes for selected targets only |
| Miri and other witnesses | Concrete UB or witness execution backstop | Only the executed and receipted witness path |
| Codecov / coverage | Execution-surface telemetry | Coverage visibility, not unsafe correctness |

The repo default remains:

```text
unsafe-review finds unsafe Rust changes missing a safety contract, guard, test,
or witness.
```

## Exception rule

There are no invisible source exceptions.

A retained exception must have a durable owner, a reason, and nearby evidence in
the appropriate policy ledger or ReviewCard-derived artifact. Companion ledgers
are acceptable only when they add behavior semantics that a source-exception
ledger cannot express, such as process, network, workflow, or release-lane
meaning.

## CI economics rule

We are not reducing CI because we want less verification. We are reducing wasted
CI so we can afford more verification where it matters.

Default PR lanes should prove the changed seam quickly. Expensive proof belongs
behind risk routing, labels, scheduled lanes, release readiness, or explicit
owner request. Optional lanes that are skipped by policy are not “passing”; they
are policy decisions with named claim boundaries.

## Agent rule

Work one review-fast PR at a time:

1. Inspect current branch, dirty state, queue posture, and source/swarm sync.
2. Choose one coherent behavior, evidence, policy, or documentation seam.
3. Keep proof near the change.
4. Run the narrow acceptance checks first, then broader gates when practical.
5. Record what the PR proves, what it does not prove, and any follow-up.
6. Clean up temporary worktrees, stale locks, and cache-heavy artifacts after
   merge.
