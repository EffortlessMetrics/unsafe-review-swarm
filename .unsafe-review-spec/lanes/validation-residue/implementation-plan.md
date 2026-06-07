# Instrument-truthfulness lane: make the coverage instrument hard to fool

## End state

0.3.4 shipped the unsafe coverage instrument and validation proved it
load-bearing. This lane ends with `unsafe-review` being a **truthful**
instrument: when it says

```text
I scanned this scope.
I found these gaps.
This receipt applies to this input.
This diff added this debt.
This report is partial or complete.
```

every line is exactly true. Not broader, not louder, not "find more" yet:

> The next useful work is not making `unsafe-review` see more. It is making
> sure what it says it saw is exactly what it actually measured.

A user on a real repo should be able to trust the instrument panel: scope and
input identity stated, files considered/skipped counted, nested repos skipped
and explainable, partial runs labelled partial with a stop reason and next
action, malformed input failing instead of silently rescoping, policy results
distinguishable from usage errors, and receipts traceable to the exact
tool/input that generated them.

## End state by capability

1. **Scope truthfulness** — nested `.git` directories and gitfile worktrees
   are skipped by default; default ignores documented; skipped nested-repo
   paths explainable in status/report; badge counts cannot inflate from
   scratch worktrees.
2. **Diff truthfulness** — a supplied `--diff` is either parsed and used or
   fails explicitly; no whole-repo fallback; no receipt claims `scope=diff`
   unless diff scope actually ran.
3. **Exit-code truthfulness** — success, policy violation, usage error,
   input/IO error, and internal error are distinct documented codes with
   machine-readable reasons.
4. **Receipt provenance** — receipts carry tool version, schema version,
   command/mode, generated_at, root identity, base/head or diff identity,
   diff hash/source, platform, dirty marker when available, and the trust
   boundary. Receipts stay traceable evidence metadata, never proof.
5. **Partial-status honesty** — timeout and `--max-cards` behave
   symmetrically: both produce a status artifact, partial artifacts say
   partial, summaries never look complete, the gate manifest carries partial
   status.
6. **Large-repo adoption guidance** — docs and `--help` explain scan cost,
   include/exclude/default ignores, partial behavior, and Bun-scale scoping.
7. **CI/workbench hygiene** — runner scratch pre-cleaned before gates, rerun
   recomputes routing, agent worktrees/watchers/artifacts cleaned or
   recorded; the tool and its workbench do not poison their own measurements.

## Operating rule

One PR, one reason, in sequence. Preflight every slice (main, open PRs,
source-divergence, dirty worktrees) with cheap bounded agent passes per the
AGENTS.md model-routing section: preflight, claim-boundary, plan refutation,
artifact verification, cleanup audit. Merge or park with evidence before the
next slice. ReviewCard stays the single projected truth; the advisory trust
boundary holds everywhere.

## PR-by-PR path

| # | Slice | Issues | Core acceptance |
|---|---|---|---|
| 1 | repo: ignore nested git checkouts and gitfile worktrees by default | #1552 | nested-repo cards not counted; skipped paths explainable; badge tests independent of local scratch state. (Workbench hygiene half — gitignoring `.claude/worktrees/` — landed in #1554; this PR is the product-level fix for all consumers.) |
| 2 | check: reject unparseable diffs instead of falling back to whole repo | #1516 | malformed diff exits nonzero with parse diagnostic; no whole-repo cards; receipt never claims diff scope that did not run |
| 3 | cli: exit-code taxonomy (policy vs usage vs input vs internal) | #1518 | no-new-debt failure distinguishable from bad input; terminal wording and JSON/status artifact name the category; docs/CLI updated. Separate PR from 2 — exit codes are an external contract |
| 4 | receipt: record tool and input provenance | #1517 | receipt traceable to exact generation context; stale/mismatched receipts detectable later; wording still avoids proof/UB-free/Miri-clean claims |
| 5 | repo: status artifact when --max-cards stops analysis | #1545 | a cap-stopped scan emits the same status artifact the timeout path emits, carrying an explicit stop reason, the cap value, cards emitted, and next-action guidance; markdown summary says partial; gate manifest never implies a full scan. Pre-step inside this slice: reconcile SPEC-0035's declared schema with the shipped timeout artifact (they have drifted — `phase`/`elapsed_seconds` vs `completed`/`elapsed_ms`) so "symmetric" is defined before fields are added; exact field names resolve against the reconciled spec, not this plan |
| 6 | docs: scan cost and large-repo scoping | #1546 | docs match the behavior PRs 1 and 5 actually shipped; copyable examples; no completeness overclaims |
| 7 | ci: pre-clean known scratch paths before gates | #1519 | bounded, logged cleanup of known temp/scratch only; never touches user-owned worktrees |
| 8 | ci: rerun recomputes route instead of reusing stale blocked route | #1513 | rerun after no_idle_runner can select fallback without an empty commit; route result visible |
| 9 | specs: align gate/baseline/receipt vocabulary with sibling schemas (ripr, cargo-allow) | #1522 #1523 #1540 #1520 | no adapter soup in ub-review for similar concepts; differences explicit and versioned; no field renames that break 0.3.4 consumers. After the truthfulness fixes, so interop aligns with correct semantics |
| 10 | analysis: stable-byte subclass hints + narrow follow-up controls | #1544 #1393 | cards say "missing stable-byte evidence", never UB; expansion only around observed Bun friction; false-positive controls land before breadth |
| 11 | ux: hypothesis + minimal confirmation cue per finding | #1394 | findings framed as hypotheses pending confirmation; concrete cue when known; no witness execution by default |
| 12 | confirm: command provenance + argv hardening | #1514 | command source visible (analyzer / manual candidate / reviewer); no silent shell interpretation; runtime_executed never fabricated |
| 13 | repair-queue: bounded applicable edits for consumers | #1542 | consumers can render one-click suggestions; unsafe-review never edits source; do_not_auto_repair / requires_human_review respected |
| 14 | xtask: bless-goldens + calibration bookkeeping toil | #1511 #1512 | golden churn controlled; calibration count updates less error-prone; no behavior change |

## Dependency PR policy

Handled in parallel, never displacing the truthfulness sequence:

- `ra_ap_syntax`: routine parser bump; merge when the full suite is green.
- `Factory-AI/droid-action`: merge only after action pinning/permission
  posture is checked.
- `signal-hook` 0.3 -> 0.4: run cancellation/status/timeout tests first.

## End-of-lane acceptance

- [ ] nested repos/worktrees do not inflate scans
- [ ] malformed diffs fail truthfully
- [ ] exit codes distinguish policy vs input vs usage vs internal failure
- [ ] receipts include provenance
- [ ] max-card cap produces a partial/status artifact
- [ ] large-repo scoping docs match behavior
- [ ] CI scratch/rerun route issues closed or explicitly parked
- [ ] interop schema alignment has a clear, versioned map
- [ ] analyzer follow-ups are issue-backed, not speculative
- [ ] validation closeout records what was proven and what remains

## Do not start with

Broad stable-byte v2, more manual candidates, another release, default
posting, policy gates, LLM behavior inside unsafe-review, badge polish, or
dependency queue-clearing. Those wait until the instrument is hard to fool.

## Boundaries

No UB-proof, UB-free, Miri-clean, site-execution, or calibrated
precision/recall claims; no default posting; no silent source edits; no
default blocking. Receipts are evidence metadata, not proof. The deterministic
gate — never a model — decides pass/fail.
