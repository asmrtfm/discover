#!/usr/bin/env bash
# test-ruby.sh — discover framework tests for Ruby lang config
source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"
load_lang ruby

FIXTURE="$TESTS_DIR/fixtures/ruby.rb"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_imports (require) ==="

imports="$(ast_grep_imports "$FIXTURE")"

assert_eq "extracts gem require" "json" "$imports"
assert_line_count "1 require" 1 "$imports"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_directive_imports (require_relative) ==="

relatives="$(ast_grep_directive_imports "$FIXTURE" require_relative)"

assert_contains "extracts relative ./foo" "./foo" "$relatives"
assert_contains "extracts relative ../bar" "../bar" "$relatives"
assert_line_count "2 require_relatives" 2 "$relatives"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (names) ==="

entities="$(ast_grep_entities "$FIXTURE" names)"

assert_contains "finds MyModule" '"name": "MyModule"' "$entities"
assert_contains "finds MyClass" '"name": "MyClass"' "$entities"
assert_contains "finds TopClass" '"name": "TopClass"' "$entities"
assert_contains "finds my_method" '"name": "my_method"' "$entities"
assert_contains "finds class_method as def self" '"kind": "def self"' "$entities"
assert_contains "finds initialize" '"name": "initialize"' "$entities"
assert_contains "finds top_level_func" '"name": "top_level_func"' "$entities"

entity_count="$(echo "$entities" | jq 'length')"
assert_eq "7 total entities" "7" "$entity_count"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (signatures) ==="

sigs="$(ast_grep_entities "$FIXTURE" signatures)"

assert_contains "class with inheritance" \
    '"text": "class TopClass < Base"' "$sigs"
assert_contains "module signature" \
    '"text": "module MyModule"' "$sigs"
assert_contains "self method signature" \
    '"text": "def self.class_method"' "$sigs"
assert_contains "method with params" \
    '"text": "def initialize(name)"' "$sigs"

report
