#!/usr/bin/env bash
# test-swift.sh — discover framework tests for Swift lang config
source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"
load_lang swift

FIXTURE="$TESTS_DIR/fixtures/swift.swift"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_imports ==="

imports="$(ast_grep_imports "$FIXTURE")"

assert_contains "extracts Foundation" "Foundation" "$imports"
assert_contains "extracts UIKit" "UIKit" "$imports"
assert_contains "extracts SwiftUI" "SwiftUI" "$imports"
assert_line_count "3 total imports" 3 "$imports"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (names) ==="

entities="$(ast_grep_entities "$FIXTURE" names)"

assert_contains "finds Identifiable protocol" '"name": "Identifiable"' "$entities"
assert_contains "finds User struct" '"name": "User"' "$entities"
assert_contains "finds NetworkError enum" '"name": "NetworkError"' "$entities"
assert_contains "finds NetworkClient class" '"name": "NetworkClient"' "$entities"
assert_contains "finds CacheManager actor" '"name": "CacheManager"' "$entities"
assert_contains "finds String extension" '"kind": "extension"' "$entities"
assert_contains "finds createApp func" '"name": "createApp"' "$entities"
assert_contains "finds configure func" '"name": "configure"' "$entities"

entity_count="$(echo "$entities" | jq 'length')"
assert_eq "9 total entities" "9" "$entity_count"

# ─────────────────────────────────────────────────────────
echo "=== entity kinds ==="

assert_contains "protocol kind" '"kind": "protocol"' "$entities"
assert_contains "struct kind" '"kind": "struct"' "$entities"
assert_contains "enum kind" '"kind": "enum"' "$entities"
assert_contains "class kind" '"kind": "class"' "$entities"
assert_contains "actor kind" '"kind": "actor"' "$entities"
assert_contains "extension kind" '"kind": "extension"' "$entities"
assert_contains "func kind" '"kind": "func"' "$entities"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (signatures) ==="

sigs="$(ast_grep_entities "$FIXTURE" signatures)"

assert_contains "struct with conformance" \
    '"text": "struct User: Identifiable, Codable {"' "$sigs"
assert_contains "enum with conformance" \
    '"text": "enum NetworkError: Error {"' "$sigs"
assert_contains "class signature" \
    '"text": "class NetworkClient {"' "$sigs"
assert_contains "actor signature" \
    '"text": "actor CacheManager {"' "$sigs"
assert_contains "protocol signature" \
    '"text": "protocol Identifiable {"' "$sigs"
assert_contains "func with return type" \
    '"text": "func createApp() -> some View {"' "$sigs"
assert_contains "async func signature" \
    '"text": "func configure(client: NetworkClient, cache: CacheManager) async {"' "$sigs"

# ─────────────────────────────────────────────────────────
echo "=== no inner method leak ==="

assert_not_contains "describe() not extracted" '"name": "describe"' "$entities"
assert_not_contains "fetch() not extracted" '"name": "fetch"' "$entities"
assert_not_contains "init not extracted" '"name": "init"' "$entities"
assert_not_contains "guest() not extracted" '"name": "guest"' "$entities"
assert_not_contains "get() not extracted" '"name": "get"' "$entities"
assert_not_contains "set() not extracted" '"name": "set"' "$entities"

report
