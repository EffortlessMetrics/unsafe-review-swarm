# UNSAFE-REVIEW-SPEC-0043: Release ergonomics — prebuilt binaries and automated promotion

Status: proposed
Owner: release/public-surface
Created: 2026-06-23
Linked proposal: #1713

## Purpose

Shorten the done→delivered path from days (manual promotion + `cargo install`)
to hours (automated promotion + prebuilt binary distribution) without removing
owner review of the promotion gate.

## Problem

The current release flow is:

1. Work builds on swarm; gates pass locally.
2. Owner manually promotes to the source/public repo.
3. Owner runs `cargo publish` for each crate in dependency order.
4. GitHub Action on the source repo picks up the new tag.
5. Users must `cargo install unsafe-review` (several minutes on a cold machine).

Steps 2–5 are manual, sequential, and take significant wall time. There is no
prebuilt binary distribution, so adopters pay the full `cargo install` build
time on every update. For CI-embedded adopters, this means each run either
re-builds the binary or caches it manually.

## Proposed slices

### Slice 1 — Prebuilt binaries via GitHub Releases

On each source-repo tag, trigger a cross-compilation build matrix and attach
the binaries to the GitHub Release:

| Target | Triple |
|---|---|
| Linux x86_64 | `x86_64-unknown-linux-gnu` |
| Linux aarch64 | `aarch64-unknown-linux-gnu` |
| macOS aarch64 | `aarch64-apple-darwin` |
| Windows x86_64 | `x86_64-pc-windows-msvc` |

The composite GitHub Action (`EffortlessMetrics/unsafe-review@v1`, SPEC-0037)
can install from the release binary rather than invoking `cargo install`,
reducing CI cold-start cost from ~3 minutes to ~5 seconds.

### Slice 2 — `cargo-binstall` support

Add `[package.metadata.binstall]` to `crates/unsafe-review/Cargo.toml` so
`cargo binstall unsafe-review` downloads the prebuilt binary from the GitHub
Release instead of compiling from source.

```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/{ version }/unsafe-review-{ target }{ archive-suffix }"
bin-dir = "unsafe-review-{ target }/unsafe-review{ binary-ext }"
```

### Slice 3 — Automated promotion

A workflow that, on a signed tag on swarm main, automatically opens a promotion
PR on the source repo and validates the source gate. Owner approves the PR;
merge triggers publish + binary build.

## Non-goals

- Does not change the detection architecture or output contracts.
- Does not change the advisory trust boundary.
- Does not remove the manual promotion step entirely — owner review of the
  promotion PR remains the gate before publish.
- Does not change the swarm→source git ceremony (documented in
  `docs/contributing/SWARM_MIRROR.md`).
- Not a performance SLA, coverage claim, or calibration claim.

## Trust boundary

Release ergonomics infrastructure only. No claim about binary correctness,
memory safety, UB-free status, Miri-clean status, or site execution. The
prebuilt binary is the same `cargo build --release` output the user would
produce locally; distribution method does not change trust posture.
