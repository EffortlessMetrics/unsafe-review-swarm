# Plans

This directory contains spec-system artifacts for PR-sized execution sequences and rollback notes.

Register governed artifacts in `.allow/artifacts/doc-artifacts.toml` so `cargo-allow check --profile spec-system` can validate their source-tree graph links.

The current draft release qualification contract is [issue #1915's release
cutline](release-cutline/UNSAFE-REVIEW-CUTLINE-1915.toml), with a human summary
in [`docs/status/RELEASE_CUTLINE.md`](../docs/status/RELEASE_CUTLINE.md). It is
not a scheduler or publication authorization.
