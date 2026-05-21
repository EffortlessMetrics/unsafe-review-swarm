# unsafe-review-cli

CLI implementation crate for `unsafe-review`.

Most users should not depend on or install this crate directly. Install the
product façade instead:

```bash
cargo install unsafe-review --locked
```

This crate owns command parsing, terminal output, artifact rendering, and the
`cargo-unsafe-review` integration binary. It depends on `unsafe-review-core` for
the ReviewCard engine and does not define an independent analyzer truth.

Current status: experimental advisory tooling. It does not run Miri,
`cargo-careful`, sanitizers, Loom, Shuttle, Kani, or Crux by default; it does
not post PR comments; and it does not enable blocking policy by default.

Repository documentation:
https://github.com/EffortlessMetrics/unsafe-review
