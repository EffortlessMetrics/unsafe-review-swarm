# ub-review exact producer/consumer smoke - 2026-08-20

Status: **partial / incompatible; not a smoke pass**

Issue: [#2116](https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/2116)

This receipt ran the `ub-review run` command path used beneath the pinned
Action route. The composite GitHub Action itself was **not run**. Its standard
installer remains pinned to unsafe-review `0.3.4`; this smoke deliberately put
the exact locally built `0.3.8` producer first on `PATH`.

The result proves semantic ingestion of the gate envelope and records an
explicit current incompatibility in comment-plan ingestion. It does not prove
card/comment or repair semantics.

## Claim boundary

No credentials were supplied. `--model-mode off` disabled model calls,
`--posting artifact-only` prevented posting, `--fail-on-gate false` kept the
command advisory, and `--tools unsafe-review` selected only this sensor. No
witness ran and no source was edited. This is not evidence of memory safety,
UB-freedom, Miri cleanliness, site execution, calibrated accuracy, a release,
or a blocking verdict. Raw retention and semantic ingestion are separate.

## Exact identities

| Item | Exact identity |
| --- | --- |
| unsafe-review source commit | `672a4c259dd9ddf8e4fa81d861b57d0d5e1254e1` |
| unsafe-review source tree | `29caa22cc974394a3d68d670f9ccc6018cb45ad2` |
| unsafe-review version | `unsafe-review 0.3.8` |
| unsafe-review binary SHA-256 | `4a11b3c27850997a7bd455e4a7fd3cf453fd9d94aaff4e2836d06c6425bceb4e` |
| ub-review source commit | `9de43c5215c8e4278cce9421b55991ced4f66065` |
| ub-review source tree | `5de342265a790de96da8b40439640746e0719042` |
| ub-review version | `ub-review 0.1.0` |
| retained ub-review command-path binary SHA-256 | `e999d8b2e125e8934c28476c1b015a9295cd7511624a067c80e4dd63eaff304c` |
| ub-review `Cargo.lock` SHA-256 | `51a9d6caec853c0b213b08ef5db5c2952e68dc3282e90b2c1e34d34945bdcda9` |
| schema-v999 shim source SHA-256 | `40b1e58a0f522d6eed9776eadb22014aa57f2c6a8cdf1e71f5eae54f1257aa0a` |
| schema-v999 shim binary SHA-256 | `7dcf33f6e0b00caba0c67d46762592f98bac2cd97c5887f979eedfe158783d06` |

The producer was built with `rtk cargo build --locked -p unsafe-review --bin
unsafe-review`. The detached consumer was built with `rtk cargo build --locked
--bin ub-review --target-dir ../ub-review-target`, then copied to the retained
`$CONSUMER_BIN` before both final runs. Earlier scratch hashes `702b0c...` and
`bed280...` named the mutable Cargo target path before later build/test commands
rewrote it; neither names the final command-path binary.

## Named paths and fixture reconstruction

```powershell
$SMOKE_WORKTREE = (Resolve-Path '.').Path
$SMOKE_ROOT = Join-Path $SMOKE_WORKTREE 'target/2116-smoke'
$FIXTURE_REPO = Join-Path $SMOKE_ROOT 'fixture-repo'
$PRODUCER_BIN = Join-Path $SMOKE_WORKTREE 'target/debug/unsafe-review.exe'
$PRODUCER_BIN_DIR = Split-Path -Parent $PRODUCER_BIN
$CONSUMER_SRC = Join-Path $SMOKE_ROOT 'ub-review-src'
$CONSUMER_BIN = Join-Path $SMOKE_ROOT 'consumer-bin/ub-review.exe'
$CONSUMER_CONFIG = Join-Path $CONSUMER_SRC 'profiles/bun-ub-v0.toml'
$POSITIVE_OUT = Join-Path $SMOKE_ROOT 'consumer-positive-final'
$NEGATIVE_OUT = Join-Path $SMOKE_ROOT 'consumer-negative-v999-final'
$SHIM_SRC = Join-Path $SMOKE_ROOT 'shim-src/main.rs'
$SHIM_BIN_DIR = Join-Path $SMOKE_ROOT 'shim-bin'
$SHIM_BIN = Join-Path $SHIM_BIN_DIR 'unsafe-review.exe'
```

The temporary Git repository was reconstructed from
`fixtures/raw_pointer_alignment`. Explicit dates reproduce the commit IDs.

```powershell
rtk git init $FIXTURE_REPO
rtk git -C $FIXTURE_REPO config user.name 'unsafe-review smoke'
rtk git -C $FIXTURE_REPO config user.email 'smoke@invalid.local'
$env:GIT_AUTHOR_DATE = '@1787256844 -0400'
$env:GIT_COMMITTER_DATE = '@1787256844 -0400'
rtk git -C $FIXTURE_REPO commit --allow-empty -m 'base: empty fixture'
$FIXTURE_SOURCE = Join-Path $SMOKE_WORKTREE 'fixtures/raw_pointer_alignment'
Get-ChildItem -LiteralPath $FIXTURE_SOURCE -Force |
  Copy-Item -Destination $FIXTURE_REPO -Recurse -Force
rtk git -C $FIXTURE_REPO add -- Cargo.toml change.diff expected.cards.json expected.comment-plan.json expected.lsp.json expected.repair-queue.json src
$env:GIT_AUTHOR_DATE = '@1787256861 -0400'
$env:GIT_COMMITTER_DATE = '@1787256885 -0400'
rtk git -C $FIXTURE_REPO commit -m 'fixture: raw pointer alignment'
```

| Item | SHA |
| --- | --- |
| base commit | `414693c8986fedfe3818ad85328d22e4e97d0cb5` |
| base tree | `4b825dc642cb6eb9a060e54bf8d69288fbee4904` |
| head commit | `292bfa7c15570dbaba70a1a0efc74c85a554900c` |
| head tree | `4eb39a0b827a80b0591a71539120ecb2d0b60402` |

Expected card ID:
`UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1`.
Its raw operation family was `raw_pointer_read`.

## Positive command and result

The standalone producer control exited `0`, and its real bundle passed the
artifact checker:

```powershell
$PRODUCER_CONTROL_OUT = Join-Path $SMOKE_ROOT 'producer-positive'
rtk $PRODUCER_BIN first-pr --root $FIXTURE_REPO --base-sha 414693c8986fedfe3818ad85328d22e4e97d0cb5 --head-sha 292bfa7c15570dbaba70a1a0efc74c85a554900c --out-dir $PRODUCER_CONTROL_OUT
rtk cargo run --locked -p xtask -- check-first-pr-artifacts $PRODUCER_CONTROL_OUT
```

The final real consumer command used the retained binary and exited `0`:

```powershell
$env:PATH = $PRODUCER_BIN_DIR + ';' + $env:PATH
$CONSUMER_ARGS = @(
  'run', '--config', $CONSUMER_CONFIG, '--profile', 'gh-runner',
  '--root', $FIXTURE_REPO,
  '--base', '414693c8986fedfe3818ad85328d22e4e97d0cb5',
  '--head', '292bfa7c15570dbaba70a1a0efc74c85a554900c',
  '--out', $POSITIVE_OUT, '--posting', 'artifact-only',
  '--mode', 'review-byok', '--run-pass', 'auto', '--model-mode', 'off',
  '--fail-on-gate', 'false', '--depth', 'standard',
  '--tools', 'unsafe-review', '--provider-policy', 'auto',
  '--lane-width', '10', '--model-timeout-sec', '300',
  '--max-inline-comments', '8', '--model-concurrency', '10',
  '--max-model-calls', '10', '--no-github-summary'
)
rtk $CONSUMER_BIN @CONSUMER_ARGS
```

| Handoff obligation | Result | Evidence |
| --- | --- | --- |
| Sensor execution | PASS | exit `0`, sensor status `ok` |
| Gate schema | PASS | routed `unsafe-review-gate/v1` |
| Tool provenance | PASS | surfaced `unsafe-review` `0.3.8` |
| Gate posture and trust | PASS | advisory trust surfaced; terminal `artifact-only` |
| Movement | PASS | new=1, worsened=0, resolved=0, inherited=0 |
| Raw card/comment/repair identity | PASS, retention only | exact card ID retained |
| Raw operation family | PASS, retention only | nested artifacts retain `raw_pointer_read` |
| Semantic comment/card ingestion | **FAIL** | producer selected one; consumer reported `Comment-plan candidates: 0` |
| Semantic repair join | **NOT PROVEN** | no parsed comment ID reached the join |
| Semantic operation-family ingestion | **NOT PROVEN** | consumer type does not bind the field |

unsafe-review 0.3.8 emits an object envelope with a `comments` array. The
pinned consumer attempts
`serde_json::from_str::<Vec<UnsafeReviewCommentPlanEntry>>` on the whole file
and silently substitutes an empty vector on failure. Raw diff text mentioning
the card or family is not semantic structured ingestion.

## Unsupported-schema negative

The negative shim is fully reconstructible from this source:

```rust
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let mut args = env::args_os().skip(1);
    let mut out_dir = None;
    while let Some(arg) = args.next() {
        if arg == "--out-dir" {
            out_dir = args.next().map(PathBuf::from);
            break;
        }
    }
    let Some(out_dir) = out_dir else {
        eprintln!("schema-v999 shim: missing --out-dir");
        std::process::exit(2);
    };
    if let Err(error) = fs::create_dir_all(&out_dir) {
        eprintln!("schema-v999 shim: create {} failed: {error}", out_dir.display());
        std::process::exit(2);
    }
    let manifest = r#"{
  "schema_version": "unsafe-review-gate/v999",
  "dialect": "unsafe-review",
  "status": "advisory",
  "summary": {"new_gaps":0,"worsened_gaps":0,"resolved_gaps":0,"inherited_gaps":0},
  "artifacts": {},
  "trust_boundary": "static unsafe-review coverage evidence; not proof, not a merge verdict",
  "tool": "unsafe-review",
  "tool_version": "0.3.8"
}
"#;
    if let Err(error) = fs::write(out_dir.join("unsafe-review-gate.json"), manifest) {
        eprintln!("schema-v999 shim: write manifest failed: {error}");
        std::process::exit(2);
    }
}
```

It was built with `rtk rustc --edition 2024 $SHIM_SRC -o $SHIM_BIN`. The
negative used the same `$CONSUMER_ARGS`, replacing `$POSITIVE_OUT` with
`$NEGATIVE_OUT` and prepending `$SHIM_BIN_DIR` instead of
`$PRODUCER_BIN_DIR` to `PATH`. The shim wrote only the requested
`unsafe-review-gate/v999` manifest, emitted no Markdown, and exited `0`.

The consumer also exited `0`, recorded `artifact-gap`, named both found
`unsafe-review-gate/v999` and known `unsafe-review-gate/v1`, stated structured
evidence was not parsed, and used status-only context. Its candidate output was
empty. The final disposition remained advisory: terminal `artifact-only`, no
blocking gap.

## Bounded artifact hashes

| Artifact | SHA-256 |
| --- | --- |
| producer-control gate | `d773adb72efb2527fe26ed74f05027c021674f718ba9885fe6a2b4022954fab3` |
| producer-control cards | `1f007dd05eec65b941d9617782fafa2657c552aa87a0f3089460aa774c011bea` |
| producer-control comment plan | `66d3d90b653708bf43c897230572ba4e2f50c3a802aac884b53c9c27ccf66d6a` |
| producer-control repair queue | `69e3f65e34a2e4d01f889788204c40dc2c9257804f45de34b82bd5763d12eca3` |
| final positive sensor status | `2f67984b4f5c6312c21d1477ae0bad7a10b9e886db32c951fc27361d587e701e` |
| final positive retained gate | `d773adb72efb2527fe26ed74f05027c021674f718ba9885fe6a2b4022954fab3` |
| final positive retained cards | `4b32282ed5975839d59fd38cdb8a9a141c55ca57b8f522cf6ef18b55f8713f8a` |
| final positive retained comment plan | `66d3d90b653708bf43c897230572ba4e2f50c3a802aac884b53c9c27ccf66d6a` |
| final positive retained repair queue | `69e3f65e34a2e4d01f889788204c40dc2c9257804f45de34b82bd5763d12eca3` |
| final positive shared context | `67649ff37fd711b6b9c03d8317302a5a1c4b8c91271ec1c6f11430f6782e3c5f` |
| final positive compiler input | `da0aada34abc8ac0120f6b69c713ae7d115554c8a5bb0c5cbe77e11fbcfd49f8` |
| final positive `candidates.ndjson` (empty file) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| final positive `review/candidates.json` (`[]`) | `4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945` |
| final positive `review/gate_outcome.json` | `9e6153c2209cf9dcdae138995514d4f8cb1f2d5aca32f6953609c630c16fb591` |
| final positive `tool-gate-outcomes.json` and review mirror | `9a6a01526a9937f97544655ab3a7f2e1b23c8beeed5532debeef1699ef244490` |
| final negative v999 manifest | `5d3f33693f7e191ea229a87a91ee70daaa67552e2f70d716e14cc846437a21bc` |
| final negative sensor status | `cb71c09b6a2bea113d2a8cc2a873a2d5fb5f8d469a10c4292aa5065cb5406c79` |
| final negative shared context | `39b96b694aa6141ea4d65f7663ca5c241d8d92e0eff57617f3389c7f006c570d` |
| final negative compiler input | `68abe383cba6926438cd1fab5302d124f8bc2e9819ac22b845fd2bd17148ef61` |
| final negative `candidates.ndjson` (empty file) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| final negative `review/candidates.json` (`[]`) | `4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945` |
| final negative `review/gate_outcome.json` | `34b4ed9d12dd68195764d0b63b09ee1e114dd8769704930d9c380a6544f0f3ea` |
| final negative `tool-gate-outcomes.json` and review mirror | `9a6a01526a9937f97544655ab3a7f2e1b23c8beeed5532debeef1699ef244490` |

The producer-control and consumer commands use different option forms, so only
byte-equal artifacts are claimed equal. Cards legitimately carry different
run-input provenance.

## Trust boundary

This receipt is producer/consumer compatibility evidence only: the gate
envelope was retained and its schema version routed, and the typed
unsupported-schema degradation path was exercised. It does not run witnesses,
execute Miri, execute the pinned composite Action, edit source, or post
comments. It does not claim UB presence or absence: not memory-safety proof,
not UB-free status, not Miri-clean status, not site-execution proof, not
calibrated precision or recall, merge readiness, policy readiness or a
blocking policy, or any publication or release readiness for either repository.

## Disposition

The gate envelope and typed unsupported-schema degradation are compatible. The
complete producer/consumer contract is not. Semantic card/comment ingestion
failed; repair joining and semantic operation-family propagation remain **NOT
PROVEN**. The `ub_review_real_consumer_smoke` policy gap therefore remains open,
and `policy/spec-coverage.toml` is byte-untouched.

The prerequisite belongs upstream: parse the current 0.3.x object envelope
fallibly, never turn malformed structured input into a silent empty list, join
repair context by exact card ID, and bind the opaque nonempty operation family.
Do not adapt the producer back to the obsolete top-level array. Rollback is
removal of this receipt and the linked SPEC-0034 status paragraph. All fixture,
source checkout, binaries, shim, and outputs are ignored scratch under
`target/2116-smoke`.
