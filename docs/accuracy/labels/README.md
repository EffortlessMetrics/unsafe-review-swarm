# Accuracy label files

Claim-scoped label ledgers live here.

Current ledgers:

- `atomic-pointer-state-witness-routes.toml`: fixture-pinned route-quality
  labels for atomic pointer state Loom/Shuttle witness routing.
- `box-from-raw-ownership.toml`: fixture-pinned obligation-level labels for
  `Box::from_raw` ownership evidence.
- `drop-in-place-box-origin.toml`: fixture-pinned obligation-level labels for
  `ptr::drop_in_place` Box-origin evidence.
- `drop-in-place-witness-routes.toml`: fixture-pinned route-quality labels for
  `ptr::drop_in_place` Miri/cargo-careful witness routing.
- `copy-nonoverlapping-valid-range.toml`: fixture-pinned
  obligation-level labels for `copy_nonoverlapping` valid-range evidence.
- `diff-site-inventory-identity.toml`: fixture-pinned owner, site-kind, and
  dedupe labels for diff unsafe site inventory behavior.
- `ffi-boundary-obligations.toml`: fixture-pinned obligation-level labels for
  FFI ABI/layout and ownership/lifetime/nullability evidence.
- `ffi-sanitizer-witness-routes.toml`: fixture-pinned route-quality labels for
  FFI sanitizer/cargo-careful witness routing.
- `get-unchecked-mut-bounds.toml`: fixture-pinned obligation-level labels for
  `get_unchecked_mut` bounds evidence.
- `inline-asm-human-review-routes.toml`: fixture-pinned route-quality labels
  for inline assembly human-deep-review witness routing.
- `local-unsafe-contract-evidence.toml`: fixture-pinned obligation-level
  labels for private/local unsafe contract evidence.
- `maybeuninit-assume-init-initialized.toml`: fixture-pinned
  obligation-level labels for `MaybeUninit::assume_init` family initialized
  evidence and Miri/cargo-careful witness routing.
- `no-card-artifact-honesty.toml`: fixture-pinned artifact-honesty labels for
  safe/import-only/cfg-only/unchanged-adjacent fixtures that should emit zero
  ReviewCards.
- `nonnull-new-unchecked-nullability.toml`: fixture-pinned obligation-level
  labels for `NonNull::new_unchecked` nullability evidence.
- `pin-unchecked-human-review-routes.toml`: fixture-pinned route-quality labels
  for `Pin::new_unchecked` human-deep-review witness routing.
- `pointer-arithmetic-bounds.toml`: fixture-pinned obligation-level labels for
  pointer arithmetic bounds evidence.
- `ptr-copy-valid-range.toml`: fixture-pinned obligation-level labels for
  `ptr::copy` valid-range evidence.
- `public-unsafe-api-safety-docs.toml`: fixture-pinned obligation-level labels
  for public unsafe API `# Safety` contract evidence.
- `raw-pointer-operation-family-smoke.toml`: fixture-pinned
  operation-family labels for raw pointer deref/read/write variants.
- `raw-pointer-read-alignment.toml`: fixture-pinned obligation-level labels for
  raw pointer read alignment evidence.
- `raw-pointer-read-bounds.toml`: fixture-pinned obligation-level labels for
  raw pointer read bounds evidence.
- `raw-pointer-write-initialized-evidence.toml`: fixture-pinned
  obligation-level labels for raw pointer write initialized evidence.
- `slice-from-raw-parts-mut-initialized.toml`: fixture-pinned
  obligation-level labels for `slice::from_raw_parts_mut` initialized-memory
  evidence.
- `static-mut-witness-routes.toml`: fixture-pinned route-quality labels for
  `static mut` Loom/Shuttle witness routing.
- `str-from-utf8-unchecked-validation.toml`: fixture-pinned obligation-level
  labels for `str::from_utf8_unchecked` UTF-8 validation evidence.
- `target-feature-human-review-routes.toml`: fixture-pinned route-quality labels
  for `#[target_feature]` human-deep-review witness routing.
- `transmute-bool-valid-value.toml`: fixture-pinned obligation-level labels for
  transmute bool valid-value evidence.
- `unreachable-unchecked-infallible-path.toml`: fixture-pinned
  obligation-level labels for `unreachable_unchecked` local infallible-path
  evidence.
- `unwrap-unchecked-valid-value-evidence.toml`: fixture-pinned
  obligation-level labels for `unwrap_unchecked` valid-value evidence.
- `unsafe-impl-send-sync-witness-routes.toml`: fixture-pinned route-quality
  labels for unsafe impl Send/Sync Loom/Shuttle witness routing.
- `unsafe-fn-call-callee-contract.toml`: fixture-pinned obligation-level labels
  for generic unsafe function call callee-contract evidence.
- `vec-from-raw-parts-capacity.toml`: fixture-pinned obligation-level labels
  for `Vec::from_raw_parts` capacity evidence.
- `vec-set-len-initialized-range.toml`: fixture-pinned obligation-level labels
  for `Vec::set_len` initialized-range evidence.
- `zeroed-valid-zero-evidence.toml`: fixture-pinned obligation-level labels
  for `mem::zeroed` valid-zero evidence and witness routing.

Each file must identify the linked policy claim, corpus metadata, sample set,
source kind, and trust boundary. Fixture-pinned ledgers do not create calibrated
accuracy claims; human-adjudicated samples still require labelers,
adjudication, and a later metric report.

Samples may pin both `expected_contract_state` and
`expected_discharge_state` when the claim is about public contract evidence.
The states are read from the ReviewCard obligation evidence, not from a
separate label-specific truth.

Samples for witness-routing claims must pin `expected_witness_route_kinds`.
Route kinds are read from the ReviewCard `witness_routes` projection.

Samples may pin `expected_owner` and `expected_site_kind` when the claim is
about ReviewCard identity or inventory behavior. These fields are read from the
matching ReviewCard `site.owner` and `site.kind`.
