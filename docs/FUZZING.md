# Fuzzing

`unsafe-review` has a manual `cargo-fuzz` harness for analyzer robustness. The
harness is not part of the default PR gate and does not prove soundness,
UB-free status, or witness success.

Run it from the repository root:

```bash
cargo install cargo-fuzz
cargo fuzz run analyze
```

The `analyze` target treats fuzz bytes as a temporary `src/lib.rs`, synthesizes a
unified diff that adds that file, runs the core analyzer with byte-selected
scope/mode/max-card settings, and checks that rendered JSON remains parseable.

The first two input bytes are interpreted as a small config header:

- bit 0 toggles `Scope::Diff` / `Scope::Repo`
- bit 1 toggles `AnalysisMode::Draft` / `AnalysisMode::Ready`
- bit 2 can emit a diff with no generated hunk (diff-tail only)
- bit 3 toggles bounded `max_cards` (`1..=128`) / `None`
- byte 2 seeds the bounded `max_cards` value

The remaining bytes are UTF-8-lossy text input for source/diff synthesis.

Inputs can optionally include this marker on its own line (LF or CRLF line endings are both accepted):

```text
---DIFF---
```

Text before the marker becomes the generated `src/lib.rs`. Text after the marker
is appended to the synthesized unified diff. This lets the corpus exercise both
source parsing and diff parsing while keeping the harness source-based and
stable-only.

The harness caps source and appended diff text to bounded byte sizes and trims
only at UTF-8 character boundaries. It writes temporary fixtures under the
system temp directory and removes them after each run.
