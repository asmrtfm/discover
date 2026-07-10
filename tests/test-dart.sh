#!/usr/bin/env bash
# test-dart.sh — discover framework tests for Dart lang config
source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"
load_lang dart

FIXTURE="$TESTS_DIR/fixtures/dart.dart"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_imports ==="

imports="$(ast_grep_imports "$FIXTURE")"

assert_contains "extracts dart:io" "dart:io" "$imports"
assert_contains "extracts dart:convert" "dart:convert" "$imports"
assert_contains "extracts flutter package" "package:flutter/material.dart" "$imports"
assert_contains "extracts self-package import" "package:my_app/models/user.dart" "$imports"
assert_contains "extracts relative ../ import" "../widgets/button.dart" "$imports"
assert_contains "extracts relative ./ import" "./helpers.dart" "$imports"
assert_line_count "7 total imports" 7 "$imports"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_directive_imports (export) ==="

exports="$(ast_grep_directive_imports "$FIXTURE" export)"

assert_contains "extracts package export" "package:my_app/base/nav.dart" "$exports"
assert_contains "extracts relative export" "../widgets/overlay.dart" "$exports"
assert_line_count "2 total exports" 2 "$exports"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_directive_imports (part) ==="

parts="$(ast_grep_directive_imports "$FIXTURE" part)"

assert_eq "extracts part directive" "user.g.dart" "$parts"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (names) ==="

entities="$(ast_grep_entities "$FIXTURE" names)"

assert_contains "finds UserRole enum" '"kind": "enum"' "$entities"
assert_contains "finds UserRole name" '"name": "UserRole"' "$entities"
assert_contains "finds Serializable mixin" '"name": "Serializable"' "$entities"
assert_contains "finds UserProfile class" '"name": "UserProfile"' "$entities"
assert_contains "finds _UserProfileState class" '"name": "_UserProfileState"' "$entities"
assert_contains "finds StringUtils extension" '"name": "StringUtils"' "$entities"
assert_contains "finds JsonMap typedef" '"name": "JsonMap"' "$entities"
assert_contains "finds Callback typedef" '"name": "Callback"' "$entities"
assert_contains "finds main function" '"name": "main"' "$entities"
assert_contains "finds fetchUser function" '"name": "fetchUser"' "$entities"
assert_contains "finds formatName function" '"name": "formatName"' "$entities"

entity_count="$(echo "$entities" | jq 'length')"
assert_eq "10 total entities" "10" "$entity_count"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (signatures) ==="

sigs="$(ast_grep_entities "$FIXTURE" signatures)"

assert_contains "class signature includes extends" \
    '"text": "class UserProfile extends StatefulWidget {"' "$sigs"
assert_contains "mixin signature" \
    '"text": "mixin Serializable {"' "$sigs"
assert_contains "extension signature includes on" \
    '"text": "extension StringUtils on String {"' "$sigs"
assert_contains "function signature has return type" \
    '"text": "String formatName(String first, String last)"' "$sigs"
assert_contains "enum signature" \
    '"text": "enum UserRole {"' "$sigs"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (no inner methods leak) ==="

assert_not_contains "build() not extracted as top-level" '"name": "build"' "$entities"
assert_not_contains "toJson() not extracted as top-level" '"name": "toJson"' "$entities"
assert_not_contains "createState() not extracted as top-level" '"name": "createState"' "$entities"

report
