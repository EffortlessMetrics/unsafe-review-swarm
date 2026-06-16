# Dogfood report: 2026-06-15 fresh-crate control-plane validation

Status: fresh-crate detector-discipline validation
Swarm commit: `e0ca89c1`
Artifact status: local, untracked under `target/dogfood-work/`

This report captures a 2026-06-15 fresh-crate dogfood pass on three crates
selected for under-represented unsafe profiles: nix (FFI/extern + cfg-gated
platform branches), simdutf8 (SIMD/target_feature density), and zerocopy
(raw-pointer/transmute/byte-cast density). These crates were scanned in repo
mode without the `--max-cards` cap, producing uncapped whole-repo diagnostics.
The corpus.toml entries use `--max-cards 50` for reproducible snapshots; the
numbers below come from the uncapped validation runs.

The purpose is to confirm that the detector-discipline control plane holds on
unseen code: that hardened operation families do not over-match on
comments/strings, that cfg-gated duplication is by-design and not a misfire,
and that the volume/classifier shape is understood before locking in the capped
targets.

## Trust boundary

This is static unsafe contract review advisory evidence only. It is not a support-tier promotion, calibration report, policy decision, safety proof,
UB-free claim, Miri-clean claim, witness result, site-execution proof, or a
calibrated precision or recall figure. No witness tools were run. The card
counts below are uncapped whole-repo scans on real code not authored by this
project; they are not a measured detection rate, a precision/recall claim, or
policy readiness evidence. The finding that zero hard false positives were
observed in the hardened families is a detector-behavior observation, not a
soundness or calibration claim.

## Scope

Three uncapped whole-repo scans run against the commits pinned in corpus.toml:

- `nix-capped` — nix-rust/nix `fb799660ccde39c22aed6f653b70e35b35bdcfe8`
- `simdutf8-capped` — rusticstuff/simdutf8 `641d57f313df57354246d2b68d4778c092e076c3`
- `zerocopy-capped` — google/zerocopy `d35c00e208880d325eaf13ec99e3a413ac163c4c`

All three runs exited 0 and produced schema-valid JSON output.

## Per-crate diagnostics

### nix @fb799660 (FFI/cfg profile)

- Files scanned: 119
- Unsafe sites detected: 1,232
- Cards emitted: 1,232
- Scan duration: ~25 s
- JSON artifact size: 6.98 MB
- Top operation families: `unsafe_fn_call` 404, `unknown` 275, `ffi` 233

**cfg-gated duplication behavior.** The `errno_location` FFI symbol appears
across 6 platform-specific branches (Linux, Android, DragonFly, FreeBSD,
etc.), each gated by `#[cfg(...)]`. The analyzer is cfg-blind by design: it
emits one card per branch, producing 6 cards for a function that resolves to
one implementation at compile time. This is correct cfg-blind behavior, not a
misfire. The duplication is a coverage/volume question at the surfacing layer,
not a correctness defect.

**No hard false positives in hardened families.** Zero cards matched comment
text, string literals, or safe-context code in the `ffi`, `zeroed`,
`get_unchecked`, `transmute`, `ptr::copy`, or `vec::set_len` families.
`unsafe fn` and `unsafe trait` definitions card as declaration-owner cards as
designed.

### simdutf8 @641d57f3 (SIMD/target_feature profile)

- Files scanned: 47
- Unsafe sites detected: 343
- Cards emitted: 343
- Scan duration: ~3 s
- Top operation families: `unknown` 199, `target_feature` 87 (25% of cards)

**target_feature density.** The `target_feature` family accounts for 25% of
cards, the highest proportion observed across this corpus. Of the 87
`target_feature` cards, 14 land on macro-argument `#[target_feature(...)]`
forms — sites where the attribute appears inside a macro invocation rather than
directly above an `unsafe fn`. These are a soft precision edge: the sites are
genuine SIMD target-feature gating patterns, not comment or string over-matches.
They represent a classifier-coarseness grouping opportunity at the surfacing
layer, not a misfire.

**No hard false positives in hardened families.** Zero comment or string
over-matches observed.

### zerocopy @d35c00e2 (raw-pointer/transmute profile)

- Files scanned: 416
- Unsafe sites detected: 917
- Cards emitted: 917
- Scan duration: ~282 s (cost outlier — large crate with many generated files)
- Top operation families: `unknown` 524, `unsafe_fn_call` 289, `transmute` 22

**Dead-code branch cards.** 2 of the 22 transmute cards land in `if false { }`
macro-stub branches — code that is syntactically present but unreachable at
compile time. The analyzer is syntax-first and does not evaluate conditional
compilation or dead-code elimination; this is expected behavior. These are not
misfires of the transmute detector; they are coverage of syntactically-present
but logically-dead paths.

**No hard false positives in hardened families.** Zero comment or string
over-matches in the `transmute`, `zeroed`, `get_unchecked`, `ffi`,
`ptr::copy`, or `vec::set_len` families.

## Cross-crate headline finding

Across 2,492 cards on three crates, zero hard false positives were observed in
the hardened families (`zeroed`, `ffi`, `get_unchecked`, `transmute`,
`ptr::copy`, `vec::set_len`). No card matched a comment, string literal, or
genuinely safe-context site in any of those families.

The dominant friction is volume and classifier coarseness, not over-matching:

- The `unknown` family dominates: 524/917 for zerocopy (57%), 199/343 for
  simdutf8 (58%), 275/1232 for nix (22%). This is a classifier-coarseness and
  grouping opportunity at the surfacing layer — a follow-on classifier-refinement
  pass could reclassify many `unknown` cards into more specific families —
  but it is not a correctness defect. Cards in the `unknown` family are
  genuine unsafe sites; they simply lack a more specific family label.
- The `--max-cards` cap blinds the surfacing layer on large crates: zerocopy
  at 917 sites is cut to 50 in the reproducible corpus snapshot. This is a
  UX/surfacing concern, not an analyzer correctness concern.
- cfg-gated per-branch duplication (nix errno_location) and macro-form
  target_feature cards (simdutf8) are soft precision edges at the surfacing
  layer, not misfires in the hardened families.

These diagnostics are advisory evidence that the hardened detector families
behave as specified on code this analyzer had never processed. They do not
constitute a precision/recall measurement, a policy readiness determination, or
a soundness claim.
