#!/usr/bin/env bash
# select-core-mode.sh — decide whether the deterministic core gate runs tests.
#
# Prints exactly one line on stdout: `with-tests` or `without-tests`.
# Exit status is always 0; every failure mode prints `with-tests`.
#
# Skip rule (explicitly reviewed, issue #2171): skip only when the changed
# path set is non-empty and EVERY path is a Markdown document (`*.md`).
# Cargo manifests, lockfiles, `.cargo` configuration, toolchain files, Rust
# sources, workflow/policy files, unknown paths, empty diffs, and unavailable
# diffs all select the full test path. A blank entry is never a real path, so
# it also selects tests (fail closed).
#
# Usage:
#   select-core-mode.sh "origin/<base>"   # git mode: NUL-delimited diff
#   select-core-mode.sh --stdin           # fixture mode: newline-delimited
# Both the CI workflow and the xtask contract fixtures execute this same
# file; there is no second implementation of the rule.
set -u

paths_file=""
stdin_mode=0
if [ "${1:-}" = "--stdin" ] && [ "$#" -eq 1 ]; then
    stdin_mode=1
elif [ "$#" -eq 1 ]; then
    paths_file="$(mktemp)" || {
        printf 'with-tests\n'
        exit 0
    }
    trap 'rm -f "$paths_file"' EXIT
    if ! git diff --name-only -z "$1...HEAD" >"$paths_file" 2>/dev/null; then
        printf 'with-tests\n'
        exit 0
    fi
else
    printf 'with-tests\n'
    exit 0
fi

count=0
result="without-tests"
if [ "$stdin_mode" -eq 1 ]; then
    while IFS= read -r path || [ -n "$path" ]; do
        count=$((count + 1))
        case "$path" in
        *.md) ;;
        *)
            result="with-tests"
            ;;
        esac
    done
else
    while IFS= read -r -d '' path || [ -n "$path" ]; do
        count=$((count + 1))
        case "$path" in
        *.md) ;;
        *)
            result="with-tests"
            ;;
        esac
    done <"$paths_file"
fi
if [ "$count" -eq 0 ]; then
    result="with-tests"
fi
printf '%s\n' "$result"
