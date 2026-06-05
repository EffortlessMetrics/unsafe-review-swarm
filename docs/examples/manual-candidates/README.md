# Manual Candidate Examples

These examples are committed smoke inputs for `unsafe-review candidate import`.
They preserve external Bun evidence as manual/advisory packets, not
analyzer-discovered ReviewCards.
Inputs may include optional `stable_byte`, `oracle_map`, `proof_mode`,
`fix_boundary`, `pr_aperture`, `fix_options`, `test_targets`, and
`do_not_touch` fields so candidate-specific proof bars, cross-language oracle
metadata, and implementer guidance project through the copy-only handoff
surfaces without adding candidates to ReviewCard-only artifacts.
Committed Bun examples must also include source-route evidence: a
`source_trace` entry with an `rg -n` command that names the same primary
`location.file` path for the file:line seam.

`cargo run --locked -p xtask -- check-manual-candidate-examples` imports every
JSON file in this directory into a disposable first-pr fixture and verifies the
manual-candidate projections.

- `textdecoder-sab.json` records a TextDecoder SharedArrayBuffer route.
- `mysql-blob-sab.json` records a Bun.SQL MySQL BLOB SharedArrayBuffer route.
- `zstd-overlap.json` records a zlib/Zstd overlapping-buffer contract route.
- `stringorbuffer-rab-stale-input.json` records an async StringOrBuffer
  resizable-ArrayBuffer stale-input route.
- `node-fs-rab-scalar-write.json` records a node:fs async scalar write
  resizable-ArrayBuffer stale-input route.
- `candidate7-sync-compression-getter-reentry.json` records Candidate 7, a Bun
  sync compression `stable-byte-source-getter-reentry` route.

Trust boundary: these examples do not execute witnesses, prove UB, prove
site execution, prove repository safety, or create policy-ready findings.
