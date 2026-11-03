# Lane proof and admission: backlog-drive rules

Status: operational record from the multi-lane backlog drive (Nov 2026).
Parent authority is [AGENT-ORCHESTRATION.md](AGENT-ORCHESTRATION.md) (proof,
review, merge) and [LIFECYCLE_SURFACE_MAP.md](LIFECYCLE_SURFACE_MAP.md)
(surface ownership). When this note conflicts with either, those documents
and `AGENTS.md` win. Each rule below records something those documents do
not already say.

## 1. Never inherit another lane's "waiting on gate" as proof

Background `check-pr`/gate jobs die with the subagent session that spawned
them. A lane reporting "waiting on gate" may be waiting on a job that will
never report. The finishing lane re-runs (or re-verifies) the gate itself
at its own exact head before PR/admission. Cross-session gate status is a
rumor, not evidence. (Refines `AGENT-ORCHESTRATION.md` §7: proof attaches
to the exact commit being considered.)

## 2. Native admission marker schema (exact)

`admission-v1` is EXACTLY 10 keys: `repository`, `pr`, `head`, `base`,
`integration`, `tree`, `coordinator` (`EffortlessSteven`), `request_id`
(fresh 32-hex), `review_id` (int), `issued_at` (now UTC). No
`verdict`/`review_kind` keys — those belong to the independent-review-v1
marker. `tree` MUST come from the merge-commit object
(`gh api .../git/commits/<integration> --jq .tree.sha`), NEVER from local
`git rev-parse HEAD^{tree}` — the two differ whenever the merge commit has
a real tree.

## 3. Verify brief SHAs; re-fetch at admission

Run `git cat-file -t` on every SHA before use in a brief — short-prefix
collisions caused rejected briefs. Always re-fetch `base`,
`merge_commit_sha`, and `tree` live at admission time; never trust a SHA
copied from an earlier packet.

## 4. Bot-review triage discipline

Verify every bot claim against the exact head with `file:line` evidence —
bots were right ~70% in this drive. Real catches included fingerprint
list-boundary collisions, unscoped pending exemptions, tag-adjacent SHA
scans, stale tests passing via the wrong branch, missing cross-panel
coverage, and unscoped CLI claims. Fix in-scope findings immediately with
discriminating tests; record out-of-scope declines with the boundary on the
thread. Never merge with an untriaged new bot thread outstanding.
(Refines `AGENT-ORCHESTRATION.md` §§8, 14.)

## 5. New tests must pin the fixed branch, not just pass

A stale-detection test once passed via the disagreement branch (wrong
bucket, right verdict). Assert the exact message/bucket, and use a fixture
that can only pass through the intended code path.

## 6. No timing-sensitive assertions

Assert on artifacts/sidecars, never on log-line ordering or
timing-sensitive stderr text.

## 7. An unimplementable spec premise stops the lane

When a spec premise proves unimplementable (e.g. a saved-JSON join through
a lossy projection that cannot carry the fidelity), STOP the lane, record
the exact blocker with `file:line` evidence, and redirect the contract —
do not silently widen scope or fake fidelity. (Lane-level form of
[spec-rails.md](spec-rails.md) "Specs are directional, not binding".)

## Claim boundary

These rules record lane-operating experience. They prove no implementation
correct, establish no merge or release readiness, and make no safety,
UB-free, Miri-clean, site-execution, or calibrated-accuracy claim.
