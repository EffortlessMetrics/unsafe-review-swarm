# unsafe-review repository state

This directory stores repository-owned coordination state for unsafe-review
development and release lanes.

- Current goal: `.unsafe-review/goals/active.toml`
- Historical goals: `.unsafe-review/goals/archive/`

Do not store product runtime output here. Runtime receipts and generated review
artifacts stay in their documented output locations.
