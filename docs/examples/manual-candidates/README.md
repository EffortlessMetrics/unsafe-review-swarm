# Manual Candidate Examples

These examples are committed smoke inputs for `unsafe-review candidate import`.
They preserve external Bun evidence as manual/advisory packets, not
analyzer-discovered ReviewCards.
Inputs may include optional `fix_options`, `test_targets`, and `do_not_touch`
arrays so candidate-specific implementer guidance projects through the copy-only
handoff surfaces without adding candidates to ReviewCard-only artifacts.

`cargo run --locked -p xtask -- check-manual-candidate-examples` imports every
JSON file in this directory into a disposable first-pr fixture and verifies the
manual-candidate projections.

- `textdecoder-sab.json` records a TextDecoder SharedArrayBuffer route.
- `mysql-blob-sab.json` records a Bun.SQL MySQL BLOB SharedArrayBuffer route.

Trust boundary: these examples do not execute witnesses, prove UB, prove
site execution, prove repository safety, or create policy-ready findings.
