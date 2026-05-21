# unsafe-review source-of-truth state

This directory stores repository-owned coordination state for unsafe-review
development and release lanes.

- Namespace index: `.unsafe-review-spec/index.toml`
- Current goal: `.unsafe-review-spec/goals/active.toml`
- Historical goals: `.unsafe-review-spec/goals/archive/`

Do not store product runtime output here. Runtime receipts stay under
`.unsafe-review/receipts/`, and generated review artifacts stay in their
documented output locations.
