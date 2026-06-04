# Bun stable-byte follow-up seed index

Status: experimental Bun dogfood backlog index

This index turns committed Bun manual candidates into lane-control seeds. It is
not a list of analyzer-discovered ReviewCards. Each row names a manual
candidate packet, a stable-byte family, proof mode, smallest first PR shape, and
ledger state so a Bun implementer can pick the next bounded slice without
re-deriving the scout route.

Triage labels come from the
[Bun stable-byte triage taxonomy](stable-byte-triage-taxonomy.md). Candidate
families come from
[`UNSAFE-REVIEW-SPEC-0027`](../specs/UNSAFE-REVIEW-SPEC-0027-manual-candidate-cards.md).

## Ledger States

- `handoff-ready`: source-routed manual packet exists and the next implementer
  action is clear.
- `fork-draft`: fix is implemented in a fork or worktree and still under local
  or fork validation.
- `upstream-open`: smallest upstreamable PR is open and maintainer-review
  gated.
- `parked-followup`: work is done and verified but not upstreamable until a
  named dependency or helper decision lands.
- `merged-upstream`: upstream PR landed and the ledger keeps receipt and
  provenance.
- `needs-refresh`: upstream/main or the fork delta moved and the route, proof,
  or patch needs recheck.

## Seeds

| Seed ID | Ledger state | Candidate family | Surface | Manual candidate | Safe JS caller | Rust/native sink | Proof mode | Suggested first PR | Owner lane | Triage labels |
|---|---|---|---|---|---|---|---|---|---|---|
| `bun-stable-byte-candidate7-sync-compression` | `handoff-ready` | `stable-byte-source-getter-reentry` | `Bun.gzipSync / Bun.deflateSync / Bun.zstdCompressSync` | `docs/examples/manual-candidates/candidate7-sync-compression-getter-reentry.json` | BufferSource input plus sync compression options getter reentry | `src/runtime/api/BunObject.rs` sync compression native read | `observable-red-green` | `sync compression getter-reentry snapshot only` | `rust1` | `observable` |
| `bun-stable-byte-textdecoder-sab` | `handoff-ready` | `stable-byte-source-sab-race` | `TextDecoder.decode` | `docs/examples/manual-candidates/textdecoder-sab.json` | SharedArrayBuffer-backed typed array decode | `src/runtime/webcore/TextDecoder.rs` slice materialization | `mutation-plus-miri` | `TextDecoder shared-byte snapshot only` | `rust2` | `non-observable`, `needs-miri-model` |
| `bun-stable-byte-stringorbuffer-rab-async` | `handoff-ready` | `stable-byte-source-rab-async` | `crypto.scrypt / crypto.pbkdf2 / Bun.zstdCompress / Bun.zstdDecompress` | `docs/examples/manual-candidates/stringorbuffer-rab-stale-input.json` | RAB-backed BufferSource resized before async completion | `src/runtime/node/types.rs` async StringOrBuffer worker read | `observable-red-green` | `non-encoded async StringOrBuffer snapshot only` | `rust3` | `observable` |
| `bun-stable-byte-node-fs-rab-scalar-write` | `handoff-ready` | `stable-byte-source-rab-async` | `node:fs scalar write / writeFile / appendFile` | `docs/examples/manual-candidates/node-fs-rab-scalar-write.json` | RAB-backed BufferSource resized before async filesystem completion | `src/runtime/node/node_fs.rs` worker-side write input read | `observable-red-green` | `node:fs scalar write input snapshot only` | `rust3` | `observable` |
| `bun-stable-byte-mysql-blob-sab` | `handoff-ready` | `stable-byte-source-sab-race` | `Bun.SQL MySQL BLOB bind` | `docs/examples/manual-candidates/mysql-blob-sab.json` | SharedArrayBuffer-backed typed array as prepared-statement BLOB | `src/sql_jsc/mysql/MySQLValue.rs` temporary raw slice | `mutation-plus-miri` | `MySQL BLOB bind-value byte stabilization only` | `rust4` | `non-observable`, `needs-miri-model` |
| `bun-stable-byte-zstd-overlap-native-ffi` | `handoff-ready` | `stable-byte-source-native-ffi-read` | `zlib Zstd _processChunk / _handle.writeSync` | `docs/examples/manual-candidates/zstd-overlap.json` | Overlapping caller-controlled ArrayBuffer input and output | `src/runtime/node/node_zlib_binding.rs` native Zstd buffer handoff | `observable-red-green` | `Zstd overlap reference boundary only` | `rust-zstd` | `observable` |

## Fixture And Control Coverage

This section records when a stable-byte seed has been converted into committed
fixture or control pressure for unsafe-review. The coverage is advisory static
review pressure only; it is not the Bun runtime witness, not patched-green
evidence, and not proof of memory safety.

| Seed ID | Positive fixture | Controls | Analyzer/support tier | Boundary |
|---|---|---|---|---|
| `bun-stable-byte-candidate7-sync-compression` | `fixtures/js_buffer_reentry_sync_compression` | `fixtures/js_buffer_reentry_options_before_capture_no_card`, `fixtures/js_buffer_reentry_recapture_after_reentry_no_card` | `JS-backed buffer reentry heuristic` | Confirms descriptor-capture-before-reentry static shape only; `observable-red-green` proof still needs external system Bun red and patched-green evidence. |
| `bun-stable-byte-stringorbuffer-rab-async` | `fixtures/js_buffer_reentry_async_helper_capture` | `fixtures/js_buffer_reentry_async_options_before_capture_no_card`, `fixtures/js_buffer_reentry_async_recapture_after_reentry_no_card` | `Stable-byte RAB async heuristic` | Confirms non-encoded async helper capture before callback reentry and later helper materialization static shape only; `observable-red-green` proof still needs external system Bun red and patched-green evidence. |
| `bun-stable-byte-node-fs-rab-scalar-write` | `fixtures/js_buffer_reentry_node_fs_rab_scalar_write` | `fixtures/js_buffer_reentry_node_fs_rab_scalar_write_scheduled_before_capture_no_card` | `Stable-byte RAB async heuristic` | Confirms node:fs-style scalar write input capture before async worker dispatch and worker-side materialization static shape only; `observable-red-green` proof still needs external system Bun red and patched-green evidence. |
| `bun-stable-byte-textdecoder-sab` | `fixtures/stable_byte_sab_borrowed_slice` | `fixtures/stable_byte_sab_snapshot_no_card` | `Stable-byte SAB race heuristic` | Confirms shared backing before borrowed raw-parts materialization static shape only; `mutation-plus-Miri/model` proof still needs external mutation pressure plus a focused model artifact. |
| `bun-stable-byte-mysql-blob-sab` | `fixtures/stable_byte_sab_mysql_blob_rawslice` | `fixtures/stable_byte_sab_mysql_blob_owned_copy_no_card` | `Stable-byte SAB race heuristic` | Confirms MySQL BLOB shared backing before temporary raw-slice storage static shape only; `mutation-plus-Miri/model` proof still needs external mutation pressure plus a focused model artifact. |
| `bun-stable-byte-zstd-overlap-native-ffi` | `fixtures/stable_byte_native_ffi_zstd_handoff` | `rtk cargo run --locked -p xtask -- check-manual-candidate-examples` | `FFI witness-routing fixture / manual-candidate smoke` | Confirms zstd-style pointer/length native handoff routes as an FFI seam and the manual candidate projects through committed smoke only; no native-FFI stable-byte analyzer heuristic is claimed, no overlap detection is claimed, and `observable-red-green` proof still needs external system Bun red and patched-green evidence. |

## How To Use

- Start from the manual candidate path and import or inspect that packet.
- Keep the suggested first PR as the aperture; do not fold in sibling families
  unless the candidate explicitly says to.
- When fixture/control coverage exists, use it as regression pressure for the
  unsafe-review heuristic or verifier only; do not treat it as witness evidence
  for the Bun bug or keep `needs-fixture` on that seed.
- Choose the proof mode before writing patch claims. Observable rows need
  system red/green evidence. Nondiscriminating rows need mutation pressure plus
  a model or Miri-shaped proof artifact.
- Move the ledger state only when the packet, receipt, upstream PR, or helper
  dependency has current evidence.

## Trust Boundary

Stable-byte follow-up seeds are advisory workflow notes tied to manual
candidates. They are not analyzer discovery, not witness execution, not a proof
of memory-safety, not UB-free status, not Miri-clean status, not site-execution
proof, not calibrated precision or recall, not policy readiness, and not a
claim that unsafe-review ran a witness, posted a comment, or edited source.
