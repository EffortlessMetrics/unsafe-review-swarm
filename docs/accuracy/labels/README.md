# Accuracy label files

Claim-scoped label ledgers live here.

Current ledgers:

- `copy-nonoverlapping-valid-range.toml`: fixture-pinned
  obligation-level labels for `copy_nonoverlapping` valid-range evidence.
- `ffi-boundary-obligations.toml`: fixture-pinned obligation-level labels for
  FFI ABI/layout and ownership/lifetime/nullability evidence.
- `ffi-sanitizer-witness-routes.toml`: fixture-pinned route-quality labels for
  FFI sanitizer/cargo-careful witness routing.
- `maybeuninit-assume-init-initialized.toml`: fixture-pinned
  obligation-level labels for `MaybeUninit::assume_init` family initialized
  evidence and Miri/cargo-careful witness routing.
- `no-card-artifact-honesty.toml`: fixture-pinned artifact-honesty labels for
  safe/import-only/cfg-only/unchanged-adjacent fixtures that should emit zero
  ReviewCards.
- `nonnull-new-unchecked-nullability.toml`: fixture-pinned obligation-level
  labels for `NonNull::new_unchecked` nullability evidence.
- `ptr-copy-valid-range.toml`: fixture-pinned obligation-level labels for
  `ptr::copy` valid-range evidence.
- `public-unsafe-api-safety-docs.toml`: fixture-pinned obligation-level labels
  for public unsafe API `# Safety` contract evidence.
- `raw-pointer-read-alignment.toml`: fixture-pinned obligation-level labels for
  raw pointer read alignment evidence.
- `str-from-utf8-unchecked-validation.toml`: fixture-pinned obligation-level
  labels for `str::from_utf8_unchecked` UTF-8 validation evidence.
- `transmute-bool-valid-value.toml`: fixture-pinned obligation-level labels for
  transmute bool valid-value evidence.
- `unsafe-impl-send-sync-witness-routes.toml`: fixture-pinned route-quality
  labels for unsafe impl Send/Sync Loom/Shuttle witness routing.
- `vec-set-len-initialized-range.toml`: fixture-pinned obligation-level labels
  for `Vec::set_len` initialized-range evidence.

Each file must identify the linked policy claim, corpus metadata, sample set,
source kind, and trust boundary. Fixture-pinned ledgers do not create calibrated
accuracy claims; human-adjudicated samples still require labelers,
adjudication, and a later metric report.

Samples may pin both `expected_contract_state` and
`expected_discharge_state` when the claim is about public contract evidence.
The states are read from the ReviewCard obligation evidence, not from a
separate label-specific truth.

Samples may pin `expected_witness_route_kinds` when the claim is about witness
routing. Route kinds are read from the ReviewCard `witness_routes` projection.
