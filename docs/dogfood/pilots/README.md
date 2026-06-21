# External Pilot Receipt Schema

Status: experimental product-usefulness evidence

External pilot receipts live at:

```text
docs/dogfood/pilots/<id>.toml
```

They record read-only runs of the public Action or an equivalent first-pr
artifact bundle on real external PRs. A receipt is adoption and usefulness
evidence: setup friction, acquisition method, selected comments, omitted cards,
runtime, artifact size, first-screen usefulness, terminology confusion, and
human or agent usefulness judgments.

It is not calibrated precision or recall, not memory-safety proof, not UB-free
status, not Miri-clean status, not site-execution evidence, not witness
adequacy, not policy readiness, and not a merge verdict. Pilot runs must be
read-only: no source edits, no witness execution, and no third-party comments or
issues.

## Local Equivalent Acquisition

Prefer the public Action when the target repository can run it read-only. For a
local-equivalent pilot receipt, keep the exact PR commits visible and avoid
shell redirection when capturing the raw diff:

```bash
gh pr view <number> --repo <owner>/<repo> --json baseRefOid,headRefOid
git -C /path/to/repo fetch origin <base-sha> <head-sha>
git -C /path/to/repo checkout --detach <head-sha>
pilot_dir="$PWD/target/external-pilots/<id>"
mkdir -p "$pilot_dir"
git -C /path/to/repo diff --binary --full-index --output="$pilot_dir/<id>.diff" <base-sha>...<head-sha>
unsafe-review pr --root /path/to/repo --base-sha <base-sha> --head-sha <head-sha> --out-dir "$pilot_dir/first-pr"
```

The `git diff --output=...` form writes Git's raw diff bytes directly to the
receipt path after its parent directory exists, using the same merge-base diff
shape as `unsafe-review pr --base-sha ... --head-sha ...`. Anchor `pilot_dir`
to the current repository root so Git's `-C /path/to/repo` does not resolve the
output path inside the external checkout. This also avoids PowerShell or shell
redirection changing the byte stream being hashed. Record the resulting path
and SHA-256 in `diff_path` and `diff_sha256`.

## File Shape

```toml
schema_version = "external-pilot/v1"
id = "bytes-pr827-2026-06-19"
status = "recorded"
date = "2026-06-19"
reviewer = "manual"
repository = "tokio-rs/bytes"
pr = 827
url = "https://github.com/tokio-rs/bytes/pull/827"
source = "local-equivalent-artifact-bundle"
acquisition_method = "repo-local cargo run"
tool_version = "0.3.8"
base_sha = "<40-char base sha>"
head_sha = "<40-char head sha>"
diff_path = "target/external-pilots/<id>/<id>.diff"
diff_sha256 = "<64-char sha256>"
trust_boundary = "Static unsafe contract review external pilot receipt; not calibrated precision or recall, not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution evidence, not witness adequacy, not policy readiness, and no third-party comments or issues were filed."

[read_only]
no_source_edits = true
no_third_party_comments = true
no_third_party_issues = true
no_witnesses_run = true

[card_inventory]
total_cards = 4
agent_ready = 0
human_only = 4

[comment_plan]
mode = "plan_only"
selected_count = 1
not_selected_count = 3
selection_reason = "bounded reviewer noise"

[gate_summary]
status = "advisory"
new_gaps = 4
worsened_gaps = 0
improved_gaps = 0
resolved_gaps = 0
inherited_gaps = 0

[run]
exit_code = 0
elapsed_seconds = 2.156
diff_bytes = 3764
artifact_count = 18
artifact_total_bytes = 201402
rust_files_changed = 3

[[artifacts]]
kind = "cards"
path = "target/external-pilots/<id>/first-pr/cards.json"
bytes = 23178
sha256 = "<64-char sha256>"

[[judgments]]
surface = "setup"
label = "setup_friction"
reason = "The diff acquisition path required explicit raw patch output."
next_step = "Keep the friction visible in the first-use backlog."
```

## Checked References

`cargo run --locked -p xtask -- check-external-pilots` verifies committed
pilot receipts:

- file names match `id`;
- the schema is `external-pilot/v1`;
- repository, PR URL, exact base/head SHAs, diff hash, and target paths are
  structurally valid;
- the read-only posture records no source edits, no witness execution, and no
  third-party comments or issues;
- comment-plan selected and omitted counts reconcile with card inventory;
- gate movement counts are advisory, non-negative values;
- runtime and artifact-size metrics are present;
- required first-pr artifacts are listed with byte sizes and sha256 hashes;
- judgments use the closed vocabulary from SPEC-0042;
- at least one setup or artifact friction row is recorded;
- the trust boundary preserves static-review, calibration, safety, UB-free,
  Miri-clean, site-execution, witness-adequacy, policy, and merge-verdict limits.

## Judgment Labels

Use one label per judgment row:

| Label | Use when | Do not infer |
|---|---|---|
| `actionable` | The pilot output helped a maintainer or reviewer take a concrete next step. | The code is unsafe, proven, or policy-blocking. |
| `correct_but_not_worth_surfacing` | A card or surface was correct but should stay quiet for the intended audience. | The card should be suppressed from structured artifacts. |
| `inherited` | The pilot surfaced pre-existing debt that should remain visible but low-noise. | The PR introduced the gap. |
| `duplicate` | Multiple cards or comments repeated the same review action. | The whole family is invalid. |
| `human_only` | The next useful action is human deep review rather than a bounded agent repair. | Agents or witnesses can never add signal later. |
| `agent_ready` | The packet gives an agent a bounded, reviewable task. | Automatic repair or source editing by default. |
| `unclear` | The reviewer could not decide usefulness without more context. | The card is correct or incorrect. |
| `incorrect` | The output was wrong-target, wrong-family, or otherwise misleading. | Global false-positive rate. |
| `missed_expected_seam` | The reviewer named an expected unsafe seam that was absent from the output. | Global recall. |
| `setup_friction` | Acquisition, checkout, diff generation, or first-run setup got in the way. | A product defect unless repeated or severe. |
| `artifact_friction` | A shipped artifact was hard to find, parse, or use. | A schema break unless the receipt shows one. |

## Trust Boundary

External pilots are static unsafe contract review product evidence. They are not
calibrated precision or recall, not memory-safety proof, not UB-free status, not
Miri-clean status, not site-execution evidence, not witness adequacy, not policy
readiness, and not a merge verdict. They do not authorize source edits, witness
execution, comments, reviews, or issue filing in a third-party repository.
