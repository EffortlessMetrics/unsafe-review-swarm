#!/usr/bin/env bash
set +e
core_exit_path="target/ci-core/core_exit-${CORE_RUN_KEY}"
core_mode="$(cat target/ci-core/core_mode)"
_step() {
  local _id="$1" _s _rc _elapsed
  shift
  _s=$(date +%s)
  "$@"
  _rc=$?
  _now=$(date +%s)
  _elapsed=$((_now - _s))
  printf '%s\t%s\t%s\n' "$_id" "$_elapsed" "$_rc" >> target/ci-core/step-status.tsv
  echo "[step-timing] ${_elapsed}s: $*"
  return $_rc
}
if [ "$core_mode" = "without-tests" ]; then
  echo "[diff-scope] Markdown-only diff — skipping test step (#2171)"
  _step fmt cargo fmt --all --check \
    && _step clippy cargo clippy --workspace --all-targets --locked -- -D warnings \
    && printf 'test\t0\tskipped\n' >> target/ci-core/step-status.tsv \
    && echo "[diff-scope] skipped test step (Markdown-only PR)" \
    && _step doc env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked \
    && _step check-pr cargo run --locked -p xtask -- check-pr
else
  _step fmt cargo fmt --all --check \
    && _step clippy cargo clippy --workspace --all-targets --locked -- -D warnings \
    && _step test env UNSAFE_REVIEW_CI_HANDOFF_DIR="${RUNNER_TEMP}/unsafe-review-structured-${CORE_RUN_KEY}" cargo run --locked -p xtask -- ci-test \
    && _step doc env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked \
    && _step check-pr cargo run --locked -p xtask -- check-pr
fi
core_rc=$?
printf '%s\n' "$core_rc" > "${core_exit_path}.tmp"
mv "${core_exit_path}.tmp" "$core_exit_path"
exit "$core_rc"
