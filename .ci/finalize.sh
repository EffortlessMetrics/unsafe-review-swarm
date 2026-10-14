#!/usr/bin/env bash
set -uo pipefail
core_exit_path="target/ci-core/core_exit-${CORE_RUN_KEY}"
core_exit="$(cat "$core_exit_path")"
echo "core_exit=${core_exit}"
if [ "${core_exit}" != "0" ]; then
  # PR-controlled commands have finished. Discard any predictable
  # staging tree, then create a private unpredictable directory under
  # this job's RUNNER_TEMP. The upload step receives only the exact
  # regular-file paths below, never this directory, so pre-populated
  # workspace extras and core.log symlinks cannot enter the artifact.
  umask 077
  failure_root="${RUNNER_TEMP}/unsafe-review-core-evidence-${CORE_RUN_KEY}"
  rm -rf "$failure_root"
  mkdir -m 700 "$failure_root"
  failure_dir="$(mktemp -d "${failure_root}/staging-XXXXXX")"
  excerpt_path="${failure_dir}/step-status.txt"
  excerpt_tmp="$(mktemp "${failure_dir}/.step-status-XXXXXX")"
  failure_summary_path="${failure_dir}/summary.md"
  failure_metadata_path="${failure_dir}/metadata.json"
  diagnostics_source="${RUNNER_TEMP}/unsafe-review-structured-${CORE_RUN_KEY}/test-diagnostics.json"
  diagnostics_path="${failure_dir}/test-diagnostics.json"
  failure_summary_source="$(mktemp "${failure_dir}/.summary-source-XXXXXX")"
  failure_summary_tmp="$(mktemp "${failure_dir}/.summary-XXXXXX")"
  failure_metadata_tmp="$(mktemp "${failure_dir}/.metadata-XXXXXX")"
  # Build the retained diagnostic exclusively in private staging from
  # the closed-vocabulary step-status sidecar. The predictable
  # workspace excerpt path is never read or written. Only known step
  # ids plus bounded decimal timing/status fields survive; arbitrary
  # command output, bare tokens, PEM blocks, and core.log content have
  # no path into the evidence.
  staging_ready="true"
  if ! {
    printf 'step_id\telapsed_seconds\texit_status\n'
    LC_ALL=C awk -F '\t' '
      NF == 3 &&
      $1 ~ /^(fmt|clippy|test|doc|check-pr)$/ &&
      $2 ~ /^[0-9]+$/ && length($2) <= 10 &&
      (($3 ~ /^[0-9]+$/ && length($3) <= 3) || $3 == "skipped") {
        print $1 "\t" $2 "\t" $3
      }
    ' target/ci-core/step-status.tsv 2>/dev/null
  } | head -n 80 | head -c 16384 > "$excerpt_tmp"; then
    staging_ready="false"
  elif ! mv "$excerpt_tmp" "$excerpt_path"; then
    staging_ready="false"
  elif [ ! -f "$excerpt_path" ] || [ -L "$excerpt_path" ]; then
    staging_ready="false"
  fi
  if [ "$staging_ready" != "true" ]; then
    echo "core failure evidence staging rejected step-status output" >&2
    rm -rf "$failure_root"
  fi
  if [ "$staging_ready" = "true" ]; then
  # Treat even the sidecar as untrusted after PR-controlled commands
  # have run: accept only its closed mode vocabulary, then select a
  # fixed command string rather than copying file content.
  core_mode="$(cat target/ci-core/core_mode 2>/dev/null || true)"
  case "$core_mode" in
    without-tests)
      core_command="cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && RUSTDOCFLAGS=\"-D warnings\" cargo doc --workspace --no-deps --locked && cargo run --locked -p xtask -- check-pr"
      ;;
    with-tests)
      core_command="cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo run --locked -p xtask -- ci-test && RUSTDOCFLAGS=\"-D warnings\" cargo doc --workspace --no-deps --locked && cargo run --locked -p xtask -- check-pr"
      ;;
    *) core_command="deterministic core suite (launch mode unavailable)" ;;
  esac
  # The xtask wrapper owns sanitization. Select only its exact regular
  # file and keep the upload bounded; no runner stream or arbitrary
  # path is copied into the artifact.
  if [ -f "$diagnostics_source" ] && [ ! -L "$diagnostics_source" ] \
    && [ "$(wc -c < "$diagnostics_source" 2>/dev/null || echo 999999)" -le 16385 ]; then
    cp --no-dereference "$diagnostics_source" "$diagnostics_path"
  fi
  if [ ! -f "$diagnostics_path" ] || [ -L "$diagnostics_path" ]; then
    rm -f "$diagnostics_path"
  elif ! cargo run --locked -p xtask -- ci-test-validate "$diagnostics_path"; then
    echo "structured test diagnostics failed schema/integrity validation" >&2
    rm -f "$diagnostics_path"
  fi
  failed_step="$(awk -F '\t' 'NR > 1 && $3 != "0" && $3 != "skipped" { print $1; exit }' "$excerpt_path" 2>/dev/null || true)"
  case "$failed_step" in
    fmt|clippy|test|doc|check-pr) ;;
    *) failed_step="unknown" ;;
  esac
  {
    echo "# Deterministic core-gate failure evidence"
    echo
    echo "- commit: \`${GITHUB_SHA}\`"
    echo "- command: \`${core_command}\`"
    echo "- exit code: \`${core_exit}\`"
    echo "- failed step: \`${failed_step}\`"
    echo "- result path: \`${core_exit_path}\`"
    echo
    echo "## Bounded allowlisted step status"
    echo
    sed 's/^/    /' "$excerpt_path"
    echo
    echo "> Closed-vocabulary diagnostic only. The raw core log is not retained."
  } > "$failure_summary_source"
  # Cap the final uploaded summary itself, including headings and
  # metadata, rather than bounding only its embedded status table.
  head -n 80 "$failure_summary_source" | head -c 16384 > "$failure_summary_tmp"
  python3 - "$failure_metadata_tmp" "$GITHUB_SHA" "$core_command" "$core_exit" "$failed_step" "$core_exit_path" "$failure_summary_path" <<'PYTHON'
import json, pathlib, sys
path, commit, command, code, step, result, summary = sys.argv[1:]
pathlib.Path(path).write_text(json.dumps(dict(schema_version="1.0", commit=commit,
    command=command, exit_code=int(code), failed_step=step, result_path=result,
    summary_path=summary)) + "\n")
PYTHON
  rm -f "$failure_summary_source"
  mv "$failure_summary_tmp" "$failure_summary_path"
  mv "$failure_metadata_tmp" "$failure_metadata_path"
  if [ -f "$failure_summary_path" ] && [ ! -L "$failure_summary_path" ] \
    && [ -f "$failure_metadata_path" ] && [ ! -L "$failure_metadata_path" ]; then
    echo "summary_path=$failure_summary_path" >> "$GITHUB_OUTPUT"
    echo "metadata_path=$failure_metadata_path" >> "$GITHUB_OUTPUT"
    if [ -f "$diagnostics_path" ] && [ ! -L "$diagnostics_path" ]; then
      echo "diagnostics_path=$diagnostics_path" >> "$GITHUB_OUTPUT"
    fi
  else
    echo "core failure evidence staging rejected non-regular output" >&2
    rm -rf "$failure_root"
  fi
  fi
  {
    echo "## Deterministic core gate"
    echo
    echo "Result: FAIL (xtask check-pr exit ${core_exit})"
    if [ "$staging_ready" = "true" ]; then
      echo
      sed 's/^/    /' "$excerpt_path"
    fi
  } >> "$GITHUB_STEP_SUMMARY"
else
  {
    echo "## Deterministic core gate"
    echo
    echo "Result: PASS (xtask check-pr exit 0)"
  } >> "$GITHUB_STEP_SUMMARY"
fi
# Advisory floor-time signal: emit wall-time + run context for
# ub-review's cost receipt (ub-review#336) to compose. This block is
# wrapped in a subshell with || true so any arithmetic or write failure
# is STRICTLY non-fatal and cannot change the gate verdict below.
(
  gate_end="$(date +%s)"
  gate_start="$(cat target/ci-core/gate_start 2>/dev/null || echo "")"
  if [ -n "${gate_start}" ] && [ "${gate_end}" -gt "${gate_start}" ] 2>/dev/null; then
    elapsed=$(( gate_end - gate_start ))
  else
    elapsed="null"
  fi
  fallback_used="false"
  if [ "${RUNNER_KIND:-github}" = "github" ]; then
    fallback_used="true"
  fi
  # Cache hit/miss/partial captured pre-build by the precontext step
  # (#1515); fall back to "unknown" if the capture file is missing or
  # holds an unexpected value.
  cache_hit_state="$(cat target/ci-core/cache_hit_state 2>/dev/null || echo "")"
  case "${cache_hit_state}" in
    hit|partial|miss) ;;
    *) cache_hit_state="unknown" ;;
  esac
  {
    printf '{"required_floor_wall_seconds":%s,"runner_kind":"%s","fallback_used":%s,"cache_hit_state":"%s"}\n' \
      "${elapsed}" "${RUNNER_KIND:-github}" "${fallback_used}" "${cache_hit_state}"
  } > target/ci-core/floor-time.json
  echo "floor-time: required_floor_wall_seconds=${elapsed} runner_kind=${RUNNER_KIND:-github} fallback_used=${fallback_used} cache_hit_state=${cache_hit_state}" \
    >> "$GITHUB_STEP_SUMMARY"
) || true
test "${core_exit}" = "0"
