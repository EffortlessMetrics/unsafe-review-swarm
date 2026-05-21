# Dogfood Corpus

This directory records the selected real-crate dogfood corpus for
`unsafe-review`.

The corpus is advisory evidence. It records repeatable targets, commands, and
expected artifact paths for real Rust crates and PR diffs. It is not a release
claim, not calibrated precision/recall measurement, and not memory-safety proof.

The manifest is [`corpus.toml`](corpus.toml). The human-facing index is
[`index.md`](index.md), with a machine-readable companion at
[`index.json`](index.json). Reviewer usefulness notes live in
[`usefulness-notes.md`](usefulness-notes.md).

## PR Diff Targets

`pr-diff` targets are repeatable only when the `root` checkout matches the
source tree expected by the saved diff. Do not record an exploratory historical
PR diff if it only produced zero cards because the local checkout had drifted
away from that PR's files or line ranges.

Record a zero-card PR diff only when the zero-card result is the intended
evidence, such as a false-positive control, and explain that in the target
`purpose`.

When an exploratory real PR exposes an unsupported class that produces zero
cards, record it as a named limitation in the dogfood handoff or objective audit
instead of counting it as an active corpus target. A zero-card result is not
evidence that the PR is safe.

When capturing a raw GitHub PR diff, use `rtk proxy` so the saved file keeps the
full patch shape:

```bash
rtk proxy gh pr diff 681 -R rust-lang/hashbrown --patch \
  > target/dogfood-work/hashbrown-pr681.raw.diff
```
