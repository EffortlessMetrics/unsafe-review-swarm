# ripr static mutation-exposure lane

`ripr` is static mutation-exposure analysis. It catches the same class of
findings mutation testing catches: weak test or weak oracle exposure. It shifts
mutation-shaped signal left because it is static and suitable for PR-time review.

## Claim boundary

`ripr` does not:

- run mutants;
- report killed or survived runtime mutation outcomes;
- prove correctness;
- replace `cargo-mutants`;
- change unsafe-review advisory findings into default CI blockers.

Runtime mutation remains the slower execution-backed backstop.

## Repo-facing surface

The durable surface should be an `xtask` wrapper, for example:

```bash
cargo run --locked -p xtask -- ripr-pr
```

The wrapper should own repo-local defaults, changed-file selection, artifact
locations, suppressions, and claim wording. Direct `ripr` invocations may be used
while bootstrapping only when the PR also documents the future wrapper boundary.

## Expected artifacts

A PR-time lane should write reviewable artifacts under a stable target directory,
such as:

```text
target/ripr/pr/pr-summary.md
target/ripr/pr/repo-exposure.json
target/ripr/pr/review.md
target/ripr/pr/agent-packet.json
target/ripr/pr/first-useful-action.md
target/ripr/pr/first-useful-action.json
```

Generated artifacts should not become a second source of truth. The source of
truth is the changed code, policy suppressions, specs, and the ReviewCard-centered
claim boundary.

## Suppressions

Suppressions belong in a policy ledger, not in ad hoc comments or workflow YAML.
Every retained suppression should name an owner, selector, reason, review date,
and evidence path. Expired suppressions should fail once the lane is promoted
from advisory to enforced policy.

## CI posture

The initial lane is advisory. Later promotion may soft-gate high-confidence new
gaps, but only after baseline cleanup, stable artifacts, and clear reviewer
wording exist.
