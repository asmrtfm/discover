#!/usr/bin/env bash
# test-crystal.sh — discover framework tests for Crystal lang config
source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"
load_lang crystal

FIXTURE="$TESTS_DIR/fixtures/crystal.cr"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_imports ==="

imports="$(ast_grep_imports "$FIXTURE")"

assert_contains "extracts stdlib require" "json" "$imports"
assert_contains "extracts stdlib sub-path" "http/client" "$imports"
assert_contains "extracts relative require" "./config" "$imports"
assert_contains "extracts glob require" "./models/*" "$imports"
assert_contains "extracts parent-relative require" "../utils" "$imports"
assert_line_count "5 total imports" 5 "$imports"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (names) ==="

entities="$(ast_grep_entities "$FIXTURE" names)"

assert_contains "finds MyApp module" '"name": "MyApp"' "$entities"
assert_contains "finds Server class" '"name": "Server"' "$entities"
assert_contains "finds Config struct" '"name": "Config"' "$entities"
assert_contains "finds Status enum" '"name": "Status"' "$entities"
assert_contains "finds AppError class" '"name": "AppError"' "$entities"
assert_contains "finds define_method macro" '"name": "define_method"' "$entities"
assert_contains "finds main def" '"name": "main"' "$entities"
assert_contains "finds self.run def" '"name": "run"' "$entities"
assert_contains "finds self.default def" '"name": "default"' "$entities"

entity_count="$(echo "$entities" | jq 'length')"
assert_eq "13 total entities" "13" "$entity_count"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (signatures) ==="

sigs="$(ast_grep_entities "$FIXTURE" signatures)"

assert_contains "class signature includes inheritance" \
    '"text": "class AppError < Exception"' "$sigs"
assert_contains "module signature" \
    '"text": "module MyApp"' "$sigs"
assert_contains "struct signature" \
    '"text": "struct Config"' "$sigs"
assert_contains "self method signature" \
    '"text": "def self.default"' "$sigs"
assert_contains "macro signature with params" \
    '"text": "macro define_method(name, content)"' "$sigs"
assert_contains "initialize with typed params" \
    '"text": "def initialize(@host : String, @port : Int32)"' "$sigs"

# ─────────────────────────────────────────────────────────
echo "=== entity kinds ==="

assert_contains "module kind correct" '"kind": "module"' "$entities"
assert_contains "class kind correct" '"kind": "class"' "$entities"
assert_contains "struct kind correct" '"kind": "struct"' "$entities"
assert_contains "enum kind correct" '"kind": "enum"' "$entities"
assert_contains "def kind correct" '"kind": "def"' "$entities"
assert_contains "macro kind correct" '"kind": "macro"' "$entities"

report
