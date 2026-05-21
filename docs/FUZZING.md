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
unified diff that adds that file, runs the core analyzer in advisory draft mode,
and checks that rendered JSON remains parseable.

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
