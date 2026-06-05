# unsafe-review-core

Core SDK and static analysis engine for `unsafe-review`.

Use this crate when embedding the ReviewCard engine programmatically. Most
command-line users should install the product facade instead:

```bash
cargo install unsafe-review
```

The public SDK centers on `AnalyzeInput`, `AnalyzeOutput`, and `ReviewCard`.
All PR summaries, SARIF, saved LSP JSON, agent packets, repo posture, outcome
reports, and receipt-aware output should project from those cards rather than
reclassifying findings independently.

Current status: experimental static unsafe contract review evidence. The engine
is source-based, stable-first, and intentionally conservative. It is not
memory-safety proof, not UB-free status, not Miri-clean status, and not a
site-execution claim unless a scoped witness receipt is attached.

Repository documentation:
https://github.com/EffortlessMetrics/unsafe-review
