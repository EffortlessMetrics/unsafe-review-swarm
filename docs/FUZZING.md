# Fuzzing

`unsafe-review` has a manual `cargo-fuzz` harness for analyzer robustness. The
harness is not part of the default PR gate and does not prove soundness,
UB-free status, or witness success.

Run it from the repository root:

```bash
cargo install cargo-fuzz
cargo fuzz run analyze
```

The `analyze` target treats fuzz bytes as a temporary fixture repo, synthesizes a
unified diff that adds `src/lib.rs`, runs the core analyzer with byte-selected
scope/mode/policy/max-card/diff-source settings, always also runs a repo scan,
and checks that ReviewCard-derived renderers keep producing parseable JSON or
non-empty text artifacts.

The first bytes are interpreted as a small config header:

- byte 0 bit 0 toggles `Scope::Diff` / `Scope::Repo`
- byte 0 bit 2 can emit a diff with no generated hunk (diff-tail only)
- byte 0 bit 3 toggles bounded `max_cards` (`1..=128`) / `None`
- byte 0 bit 4 toggles `include_unchanged_tests`
- byte 0 bit 5 toggles `DiffSource::Text` / `DiffSource::File`
- byte 0 bit 6 toggles optional witness receipt audit rendering
- byte 1 seeds the bounded `max_cards` value
- byte 2 selects `AnalysisMode::Instant`, `Draft`, `Ready`, or `Repo`
- byte 3 selects `PolicyMode::Advisory`, `NoNewDebt`, or `Blocking`

For inputs with at least two bytes, up to the first four bytes are consumed as
config. Shorter inputs use the default config. The remaining bytes are
UTF-8-lossy text input for source/diff/test synthesis. Legacy corpus entries
without an intentional binary header are still useful because their first bytes
become deterministic config bytes.

Inputs can optionally include this marker on its own line (LF or CRLF line endings are both accepted):

```text
---DIFF---
```

Text before the marker becomes the generated `src/lib.rs`. Text after the marker
is appended to the synthesized unified diff. Inputs can also include a
`---TESTS---` marker after the source/diff material; text after that marker is
written to `tests/fuzz.rs` so the harness can exercise unchanged-test discovery
when that config bit is enabled. This lets the corpus exercise source parsing,
diff parsing, file-backed diffs, projection rendering, policy reporting, and
receipt-audit rendering while keeping the harness source-based and stable-only.

The harness caps source and appended diff text to bounded byte sizes and trims
only at UTF-8 character boundaries. It writes temporary fixtures under the
system temp directory and removes them after each run. When bit 5 is set, the
synthesized diff is also written inside that temporary fixture and analyzed
through the file-backed diff path.
