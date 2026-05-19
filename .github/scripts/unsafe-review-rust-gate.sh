set -euo pipefail

started_at="$(date +%s)"

echo "repo=${GITHUB_REPOSITORY:-unknown}"
echo "workflow=${GITHUB_WORKFLOW:-unknown}"
echo "run_id=${GITHUB_RUN_ID:-unknown}"
echo "runner_name=${RUNNER_NAME:-unknown}"
echo "runner_os=${RUNNER_OS:-unknown}"
echo "base_ref=${BASE_REF:-main}"

cleanup_self_hosted() {
  if [[ "${UNSAFE_REVIEW_SELF_HOSTED:-}" != "1" ]]; then
    return
  fi

  echo "cleanup_result=starting"
  if [[ "${TMPDIR:-}" == /mnt/ci-scratch/tmp/* ]]; then
    sudo rm -rf "${TMPDIR}" 2>/dev/null || rm -rf "${TMPDIR}" || true
  fi
  if [[ "${CARGO_TARGET_DIR:-}" == /mnt/ci-scratch/target/* ]]; then
    sudo rm -rf "${CARGO_TARGET_DIR}" 2>/dev/null || rm -rf "${CARGO_TARGET_DIR}" || true
  fi
  echo "cleanup_result=finished"
  df -h /mnt/ci-scratch /mnt/ci-cache 2>/dev/null || true
}

trap cleanup_self_hosted EXIT

if [[ "${UNSAFE_REVIEW_SELF_HOSTED:-}" == "1" ]]; then
  mkdir -p "${TMPDIR}" "${CARGO_TARGET_DIR}" "${CARGO_HOME}" "${SCCACHE_DIR}"
  df -h /mnt/ci-scratch /mnt/ci-cache

  scratch_free_mb="$(df -Pm /mnt/ci-scratch | awk 'NR == 2 { print $4 }')"
  cache_free_mb="$(df -Pm /mnt/ci-cache | awk 'NR == 2 { print $4 }')"
  echo "disk_free_scratch_mb=${scratch_free_mb}"
  echo "disk_free_cache_mb=${cache_free_mb}"

  if [[ "${scratch_free_mb}" -lt 80000 ]]; then
    echo "::error::/mnt/ci-scratch has less than 80 GB free"
    exit 1
  fi
  if [[ "${cache_free_mb}" -lt 80000 ]]; then
    echo "::warning::/mnt/ci-cache has less than 80 GB free"
  fi

  if [[ -x /usr/local/cargo/bin/sccache ]]; then
    export RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
  fi
fi

if ! git rev-parse --verify --quiet "origin/${BASE_REF:-main}" >/dev/null; then
  git fetch --no-tags origin "${BASE_REF:-main}"
fi

cargo fmt --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- check-pr

cargo build --locked -p unsafe-review
mkdir -p target/unsafe-review
./target/debug/unsafe-review check --base "origin/${BASE_REF:-main}" --format json --out target/unsafe-review/cards.json
./target/debug/unsafe-review check --base "origin/${BASE_REF:-main}" --format pr-summary --out target/unsafe-review/pr-summary.md
./target/debug/unsafe-review check --base "origin/${BASE_REF:-main}" --format sarif --out target/unsafe-review/cards.sarif
./target/debug/unsafe-review check --base "origin/${BASE_REF:-main}" --format comment-plan --out target/unsafe-review/comment-plan.json
cargo run --locked -p xtask -- check-advisory-artifacts target/unsafe-review

if command -v sccache >/dev/null 2>&1; then
  sccache --show-stats || true
elif [[ -x /usr/local/cargo/bin/sccache ]]; then
  /usr/local/cargo/bin/sccache --show-stats || true
fi

finished_at="$(date +%s)"
echo "duration_seconds=$((finished_at - started_at))"
echo "artifact_cards_json=target/unsafe-review/cards.json"
echo "artifact_pr_summary=target/unsafe-review/pr-summary.md"
echo "artifact_sarif=target/unsafe-review/cards.sarif"
echo "artifact_comment_plan=target/unsafe-review/comment-plan.json"
