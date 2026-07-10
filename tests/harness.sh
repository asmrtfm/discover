#!/usr/bin/env bash
# harness.sh — shared test helpers for discover framework tests.
#
# Source this at the top of any test-<lang>.sh file:
#   source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DISCOVER_ROOT="$(cd "$TESTS_DIR/.." && pwd)"
PASS=0
FAIL=0

assert_eq() {
    local label="$1" expected="$2" actual="$3"
    if [ "$expected" = "$actual" ]; then
        echo "  PASS: $label"
        ((PASS++))
    else
        echo "  FAIL: $label"
        echo "    expected: $expected"
        echo "    actual:   $actual"
        ((FAIL++))
    fi
}

assert_contains() {
    local label="$1" needle="$2" haystack="$3"
    if echo "$haystack" | grep -qF "$needle"; then
        echo "  PASS: $label"
        ((PASS++))
    else
        echo "  FAIL: $label"
        echo "    expected to contain: $needle"
        echo "    actual: $haystack"
        ((FAIL++))
    fi
}

assert_not_contains() {
    local label="$1" needle="$2" haystack="$3"
    if ! echo "$haystack" | grep -qF "$needle"; then
        echo "  PASS: $label"
        ((PASS++))
    else
        echo "  FAIL: $label"
        echo "    expected NOT to contain: $needle"
        ((FAIL++))
    fi
}

assert_exit() {
    local label="$1" expected_code="$2"
    shift 2
    "$@" >/dev/null 2>/dev/null
    local actual_code=$?
    if [ "$actual_code" -eq "$expected_code" ]; then
        echo "  PASS: $label"
        ((PASS++))
    else
        echo "  FAIL: $label"
        echo "    expected exit code: $expected_code"
        echo "    actual exit code:   $actual_code"
        ((FAIL++))
    fi
}

assert_json_eq() {
    local label="$1" expected="$2" actual="$3"
    local norm_expected norm_actual
    norm_expected="$(echo "$expected" | jq -S '.' 2>/dev/null)"
    norm_actual="$(echo "$actual" | jq -S '.' 2>/dev/null)"
    if [ "$norm_expected" = "$norm_actual" ]; then
        echo "  PASS: $label"
        ((PASS++))
    else
        echo "  FAIL: $label"
        echo "    expected: $norm_expected"
        echo "    actual:   $norm_actual"
        ((FAIL++))
    fi
}

assert_line_count() {
    local label="$1" expected="$2" actual_text="$3"
    local count
    count="$(echo "$actual_text" | grep -c . || true)"
    if [ "$count" -eq "$expected" ]; then
        echo "  PASS: $label"
        ((PASS++))
    else
        echo "  FAIL: $label"
        echo "    expected $expected lines, got $count"
        ((FAIL++))
    fi
}

# Load a lang config for unit-level function testing
load_lang() {
    local lang="$1"
    local config="$DISCOVER_ROOT/langs/${lang}.sh"
    [ -f "$config" ] || { echo "FATAL: no lang config at $config" >&2; exit 2; }
    source "$config"
}

# Report results and exit
report() {
    echo ""
    echo "─────────────────────────────────────"
    echo "PASS: $PASS  FAIL: $FAIL"
    if [ "$FAIL" -gt 0 ]; then
        echo "SOME TESTS FAILED"
        exit 1
    else
        echo "ALL TESTS PASSED"
        exit 0
    fi
}
