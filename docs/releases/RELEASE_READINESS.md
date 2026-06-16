# Release readiness — corpus validation checklist

This is the durable pre-publish corpus-validation checklist. The full release
ceremony (version bump, promotion, publish, receipt, mirror-back) lives in
[CRATES_IO_PATCH_RELEASE.md](CRATES_IO_PATCH_RELEASE.md) and the current
release-readiness runbook under `docs/handoffs/`. This document covers only the
corpus-validation subset that must run at release cadence but is too slow to run
on every PR.

Implements the layer-3/4 release/nightly cadence clause from
`docs/specs/UNSAFE-REVIEW-SPEC-0042-corpus-validation-taxonomy.md`.

---

## Every-PR corpus gates (enforced in check-pr — confirm green at release)

These gates run on every PR via `cargo run -p xtask -- check-pr`. At release
time, confirm `main` is green before promotion; do not re-run them separately
unless the branch diverges.

- `check-detector-contracts` — per-family D1-D5 discipline contracts
- `check-stance-decisions` — stance ledger integrity
- `check-stance-coverage` — every stance has fixture evidence
- `check-spec-coverage` — spec obligations mapped to corpus cases
- `check-fixtures` — fixture manifest completeness
- `check-calibration` — fixture-to-expected-cards calibration
- `check-fixture-surface-parity` — multi-surface goldens per exemplar
- `check-real-pr-corpus` — real-PR movement corpus bounded invariants
- `check-dogfood` — dogfood manifest schema validation

---

## Release-readiness-only heavy checks (NOT every-PR — run before publish)

The following checks require network access, real repository clones, or long
runtimes (zerocopy alone ~282s). They are off the every-PR path by design.

### 1. Execute the real-repo corpus

```bash
cargo run --locked -p xtask -- dogfood-exec --strict
```

- Clones pinned commits from `docs/dogfood/corpus.toml`, runs the tool, and
  validates bounded invariants: no crash, schema-valid output, card counts within
  range.
- **Pass criteria:** 0 `run_failed` targets, 0 `schema_failed` targets.
  `clone_failed` entries caused by transient network conditions are tolerable;
  re-run to confirm.
- This command needs network access and may take several minutes. Use `--target
  <id>` to isolate a single corpus target if re-checking after a failure.

### 2. Confirm the dogfood report shows no regression

Open the most recent report in `docs/dogfood/reports/` and compare to the prior
release report.

- The `unknown`-family dominance and card-cap friction are known characterization
  findings (documented), not regressions.
- A regression is: a previously-absent `run_failed` or `schema_failed` entry, a
  card-count range breach outside tolerance, or a new crash class not present in
  the prior report.
- Record the report path and run date in the release receipt.

---

## Trust boundary

Real-repo and real-PR corpora are diagnostics, not proof. Corpus results must
not be stated as, and must not imply:

- memory-safety proof or that the tool "proves" code correct,
- UB-free status for any analyzed site or corpus run,
- Miri-clean status (unless a separate witness receipt is attached),
- site-execution or witness execution proof,
- calibrated precision or recall (fixture calibration is obligation-level
  evidence for specific detection shapes, not a global accuracy claim).

No corpus result blocks merges or posts comments by default. Corpus execution is
advisory infrastructure; it feeds evidence back into the ledgers. The ReviewCard
is the single truth object; all output surfaces project from it.

SPEC-0042 §Claim boundary and trust boundary applies to every layer.
