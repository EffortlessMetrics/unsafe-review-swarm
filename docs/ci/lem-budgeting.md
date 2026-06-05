# Linux-equivalent minute budgeting

Linux-equivalent minutes (LEM) are the repo's CI fuel gauge:

```text
LEM = wall-clock minutes × runner multiplier
```

LEM makes different runners comparable and keeps high-volume agent-assisted work
from spending deep-validation budget on every review-fast PR.

## Runner multipliers

Use these planning multipliers unless a policy ledger records a repo-specific
estimate:

| Runner or lane class | Multiplier |
| --- | ---: |
| Ubuntu/Linux | 1.0 |
| Windows | 2.0 |
| macOS | 10.0 |
| Docker-heavy lanes | 6.0 |
| GPU lanes | 6.0 |
| External AI review | 1.0 plus service-specific cost notes |

## Budget posture

Default PRs should be small enough that reviewers and agents can iterate without
waiting on broad proof that the PR does not claim. A practical posture is:

- preferred default budget: about 25 LEM;
- default soft limit: about 35 LEM;
- elevated label-selected limit: about 75 LEM;
- hard limit requiring explicit owner acknowledgement: about 125 LEM.

These numbers are planning rails, not product claims. If the repo records learned
actuals in policy, use the learned value.

## What belongs in a PR estimate

A CI plan should name:

- selected default lanes;
- selected label/main/nightly/release lanes;
- expected runner class;
- cache assumptions;
- expected artifacts or receipts;
- whether branch protection changes;
- what the PR proves and does not prove.

## Spending rule

Spend CI where it buys evidence:

- format, check, lint, targeted tests, and artifact-integrity checks are good
  default spend;
- coverage is execution-surface telemetry and should not block ordinary PRs by
  threshold;
- mutation and Miri are valuable backstops but should target risk seams or run on
  scheduled/release lanes;
- CodeQL or similar long security scans should be policy-visible lanes, not
  hidden leaf requirements for every tiny PR.
