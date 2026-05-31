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
scope/mode/max-card/diff-source settings, always also runs a repo scan, and
checks that ReviewCard-derived renderers keep producing parseable JSON or
non-empty text artifacts.

The first two input bytes are interpreted as a small config header:

- bit 0 toggles `Scope::Diff` / `Scope::Repo`
- bit 1 toggles the default mode from `AnalysisMode::Draft` to `AnalysisMode::Ready`
- bit 2 can emit a diff with no generated hunk (diff-tail only)
- bit 3 toggles bounded `max_cards` (`1..=128`) / `None`
- bit 4 extends mode selection to `AnalysisMode::Instant` (with bit 1 off) or `AnalysisMode::Repo` (with bit 1 on)
- bit 5 toggles `DiffSource::Text` / `DiffSource::File` for the synthesized diff
- byte 1 seeds the bounded `max_cards` value

The remaining bytes after the two-byte header are UTF-8-lossy text input for
source/diff synthesis. Legacy corpus entries without an intentional binary header
are still useful because their first two source bytes become deterministic config
bytes.

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
system temp directory and removes them after each run. When bit 5 is set, the
synthesized diff is also written inside that temporary fixture and analyzed
through the file-backed diff path.
