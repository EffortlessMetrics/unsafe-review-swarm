#!/usr/bin/env bash
# Repository proof for the admitted central Rust standard workflow.
set -uo pipefail
: "${RUNNER_TEMP:?central disposable runtime is required}"
: "${CARGO_TARGET_DIR:?central Cargo target directory is required}"
: "${GITHUB_SHA:?exact event source is required}"
: "${GITHUB_RUN_ID:?native run identity is required}"
: "${GITHUB_RUN_ATTEMPT:?native attempt identity is required}"
[[ "$CARGO_TARGET_DIR" == "$RUNNER_TEMP/cargo-target" ]] || exit 78
[[ "$GITHUB_RUN_ID" =~ ^[0-9]+$ && "$GITHUB_RUN_ATTEMPT" =~ ^[1-9][0-9]*$ ]] || exit 78
[[ "$(git rev-parse HEAD)" == "$GITHUB_SHA" ]] || exit 78
export CORE_RUN_KEY="${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
export CARGO_INCREMENTAL=0 CARGO_TERM_COLOR=always
case "${EM_CI_EXECUTION_SELF_HOSTED:-}" in
  true) export RUNNER_KIND=self-hosted ;;
  false) export RUNNER_KIND=github ;;
  *) exit 78 ;;
esac
export RUST_CACHE_HIT=unknown
# target/ci-core is a runtime receipt directory; compiled outputs stay in the
# central CARGO_TARGET_DIR. Never redirect or collect the entire Cargo target.
mkdir -p target/ci-core || exit 78
core_exit_path="target/ci-core/core_exit-${CORE_RUN_KEY}"
rm -f target/ci-core/core_exit "$core_exit_path" "${core_exit_path}.tmp" \
  target/ci-core/step-status.tsv target/ci-core/core_mode
base_ref="${GITHUB_BASE_REF:-main}"
core_mode="$(bash .github/scripts/select-core-mode.sh "origin/${base_ref}")"
printf '%s\n' "$core_mode" > target/ci-core/core_mode
printf 'unknown\n' > target/ci-core/cache_hit_state
{
  printf '# Deterministic core-gate context\n\n'
  printf 'event: %s\nsha: %s\nmode: %s\n' "$GITHUB_EVENT_NAME" "$GITHUB_SHA" "$core_mode"
} > target/ci-core/precontext.md
# Socket inactivity timeouts do not bound DNS/connect/body elapsed time. Own
# each complete metadata process group, including output drain, for at most
# five seconds plus one second of termination grace. No caller time override.
metadata_budget() {
  python3 .ci/supervise.py --seconds 5 --grace 1 -- python3 .ci/native_deadline.py "$1"
}
# Subtract all time since native run creation, including queue/setup. Reserve
# 180 seconds for termination grace, finalization and external cleanup; this
# entrypoint cannot itself enforce phases owned by the immutable central job.
proof_seconds="$(metadata_budget proof)"
metadata_exit=$?
[[ "$metadata_exit" == 0 ]] || exit "$metadata_exit"
[[ "$proof_seconds" =~ ^[1-9][0-9]*$ && "$proof_seconds" -le 1020 ]] || exit 78
date +%s > target/ci-core/gate_start
python3 .ci/supervise.py --seconds "$proof_seconds" --grace 30 -- bash .ci/core.sh > target/ci-core/core.log 2>&1
core_exit=$?
# The synchronous process status is authoritative even if it died before
# recording its own receipt. Timeout/cancellation can never become success.
if ! { printf '%s\n' "$core_exit" > "${core_exit_path}.tmp" \
    && mv "${core_exit_path}.tmp" "$core_exit_path"; }; then
  echo '::error::core result receipt publication failed' >&2
  # Missing result evidence must never turn a successful command into a green
  # proof. Preserve an existing product/timeout failure as the primary status.
  [[ "$core_exit" != 0 ]] || core_exit=78
  exit "$core_exit"
fi
# Refresh native origin and remaining time after proof. Metadata failure cannot
# make a successful proof green; an earlier product failure remains primary.
finalize_seconds="$(metadata_budget finalize)"
metadata_exit=$?
if [[ "$metadata_exit" != 0 ]]; then
  [[ "$core_exit" != 0 ]] || core_exit="$metadata_exit"
  exit "$core_exit"
fi
[[ "$finalize_seconds" =~ ^[1-9][0-9]*$ && "$finalize_seconds" -le 1144 ]] || {
  [[ "$core_exit" != 0 ]] || core_exit=78
  exit "$core_exit"
}
[[ "$finalize_seconds" -le 110 ]] || finalize_seconds=110
# Diagnostic failure must not overwrite the original product result.
python3 .ci/supervise.py --seconds "$finalize_seconds" --grace 10 -- bash .ci/finalize.sh || true
# Never report success after the absolute deadline, even if diagnostics timed out.
metadata_budget result >/dev/null
metadata_exit=$?
if [[ "$metadata_exit" != 0 ]]; then
  [[ "$core_exit" != 0 ]] || core_exit="$metadata_exit"
fi
exit "$core_exit"
