#!/usr/bin/env bash
# run.sh — run all discover framework tests.
#
# Usage:
#   bash tests/run.sh           # run all
#   bash tests/run.sh dart bash  # run specific languages

set -euo pipefail

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOTAL_PASS=0
TOTAL_FAIL=0
LANGS_PASS=0
LANGS_FAIL=0

if [ $# -gt 0 ]; then
    langs=("$@")
else
    langs=()
    for f in "$TESTS_DIR"/test-*.sh; do
        [ -f "$f" ] || continue
        lang="$(basename "$f" .sh)"
        lang="${lang#test-}"
        langs+=("$lang")
    done
fi

for lang in "${langs[@]}"; do
    test_file="$TESTS_DIR/test-${lang}.sh"
    if [ ! -f "$test_file" ]; then
        echo "SKIP: no test file for $lang"
        continue
    fi

    echo ""
    echo "═══════════════════════════════════════"
    echo "  $lang"
    echo "═══════════════════════════════════════"

    if bash "$test_file"; then
        LANGS_PASS=$((LANGS_PASS + 1))
    else
        LANGS_FAIL=$((LANGS_FAIL + 1))
    fi
done

echo ""
echo "═══════════════════════════════════════"
echo "  SUMMARY: $LANGS_PASS languages passed, $LANGS_FAIL failed"
echo "═══════════════════════════════════════"

[ "$LANGS_FAIL" -eq 0 ] || exit 1
