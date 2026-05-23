# Accuracy label files

Claim-scoped label ledgers live here.

Current ledgers:

- `public-unsafe-api-safety-docs.toml`: fixture-pinned obligation-level labels
  for public unsafe API `# Safety` contract evidence.
- `raw-pointer-read-alignment.toml`: fixture-pinned obligation-level labels for
  raw pointer read alignment evidence.
- `transmute-bool-valid-value.toml`: fixture-pinned obligation-level labels for
  transmute bool valid-value evidence.
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
