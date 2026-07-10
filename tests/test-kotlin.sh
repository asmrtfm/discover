#!/usr/bin/env bash
# test-kotlin.sh — discover framework tests for Kotlin lang config
source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"
load_lang kotlin

FIXTURE="$TESTS_DIR/fixtures/kotlin.kt"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_imports ==="

imports="$(ast_grep_imports "$FIXTURE")"

assert_contains "extracts stdlib import" "kotlin.collections.List" "$imports"
assert_contains "extracts specific class import" "com.example.models.User" "$imports"
assert_contains "extracts wildcard import with asterisk" "com.example.utils.*" "$imports"
assert_contains "extracts aliased import with as keyword" "com.example.network.ApiClient as Client" "$imports"
assert_line_count "4 total imports" 4 "$imports"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (names) ==="

entities="$(ast_grep_entities "$FIXTURE" names)"

assert_contains "finds UserRole enum" '"kind": "enum"' "$entities"
assert_contains "finds UserRole name" '"name": "UserRole"' "$entities"
assert_contains "finds UserProfile data class" '"kind": "data_class"' "$entities"
assert_contains "finds UserProfile name" '"name": "UserProfile"' "$entities"
assert_contains "finds Serializable interface" '"kind": "interface"' "$entities"
assert_contains "finds Serializable name" '"name": "Serializable"' "$entities"
assert_contains "finds Result sealed class" '"kind": "sealed_class"' "$entities"
assert_contains "finds Result name" '"name": "Result"' "$entities"
assert_contains "finds AppConfig object" '"kind": "object"' "$entities"
assert_contains "finds AppConfig name" '"name": "AppConfig"' "$entities"
assert_contains "finds Registry class" '"name": "Registry"' "$entities"
assert_contains "finds BaseRepository abstract class" '"kind": "abstract_class"' "$entities"
assert_contains "finds BaseRepository name" '"name": "BaseRepository"' "$entities"
assert_contains "finds Inject annotation" '"kind": "annotation"' "$entities"
assert_contains "finds Inject name" '"name": "Inject"' "$entities"
assert_contains "finds main function" '"name": "main"' "$entities"
assert_contains "finds fetchUser function" '"name": "fetchUser"' "$entities"
assert_contains "finds formatName function" '"name": "formatName"' "$entities"
assert_contains "finds MAX_CONNECTIONS val" '"name": "MAX_CONNECTIONS"' "$entities"
assert_contains "finds JsonMap typealias" '"name": "JsonMap"' "$entities"
assert_contains "finds Callback typealias" '"name": "Callback"' "$entities"

entity_count="$(echo "$entities" | jq 'length')"
assert_eq "14 total entities" "14" "$entity_count"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (signatures) ==="

sigs="$(ast_grep_entities "$FIXTURE" signatures)"

assert_contains "enum signature" \
    '"text": "enum class UserRole { ADMIN, EDITOR, VIEWER }"' "$sigs"
assert_contains "data class signature with supertypes" \
    '"text": "data class UserProfile(val name: String, val age: Int) : Serializable {"' "$sigs"
assert_contains "interface signature" \
    '"text": "interface Serializable {"' "$sigs"
assert_contains "sealed class signature with generics" \
    '"text": "sealed class Result<out T> {"' "$sigs"
assert_contains "object signature" \
    '"text": "object AppConfig {"' "$sigs"
assert_contains "abstract class signature with generics" \
    '"text": "abstract class BaseRepository<T> {"' "$sigs"
assert_contains "suspend function signature" \
    '"text": "suspend fun fetchUser(id: String): UserProfile {"' "$sigs"
assert_contains "typealias signature" \
    '"text": "typealias JsonMap = Map<String, Any>"' "$sigs"
assert_contains "val property signature" \
    '"text": "val MAX_CONNECTIONS = 10"' "$sigs"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (no inner members leak) ==="

assert_not_contains "nested Success not extracted as top-level" '"name": "Success"' "$entities"
assert_not_contains "nested Error not extracted as top-level" '"name": "Error"' "$entities"
assert_not_contains "toJson() not extracted as top-level" '"name": "toJson"' "$entities"
assert_not_contains "create() not extracted as top-level" '"name": "create"' "$entities"
assert_not_contains "register() not extracted as top-level" '"name": "register"' "$entities"
assert_not_contains "findById() not extracted as top-level" '"name": "findById"' "$entities"
assert_not_contains "companion object not extracted" '"kind": "companion"' "$entities"
assert_not_contains "nested const not extracted" '"name": "VERSION"' "$entities"

# ─────────────────────────────────────────────────────────
echo "=== resolve_import (skip list) ==="

# Each tests a distinct branch in the stdlib/platform case statement
assert_eq "skips kotlin.* stdlib" "" "$(resolve_import "kotlin.collections.List" "")"
assert_eq "skips kotlinx.* extensions" "" "$(resolve_import "kotlinx.coroutines.launch" "")"
assert_eq "skips java.* stdlib" "" "$(resolve_import "java.io.File" "")"
assert_eq "skips android.* platform" "" "$(resolve_import "android.app.Application" "")"
assert_eq "skips androidx.* jetpack" "" "$(resolve_import "androidx.lifecycle.ViewModel" "")"

# ─────────────────────────────────────────────────────────
echo "=== resolve_import (file resolution) ==="

# Set up a temp project tree to verify actual path resolution
_RESOLVE_TMP="$(mktemp -d)"
trap 'rm -rf "$_RESOLVE_TMP"' EXIT

REPO_ROOT="$_RESOLVE_TMP"
SRC_DIR_REL="src/main/kotlin"
PACKAGE_NAME="com.example.app"

# Create a source file at the expected path
mkdir -p "$_RESOLVE_TMP/src/main/kotlin/com/example/app/models"
echo "package com.example.app.models" > "$_RESOLVE_TMP/src/main/kotlin/com/example/app/models/User.kt"
echo "package com.example.app.models" > "$_RESOLVE_TMP/src/main/kotlin/com/example/app/models/Order.kt"
echo "package com.example.app" > "$_RESOLVE_TMP/src/main/kotlin/com/example/app/Main.kt"

# Specific class import resolves to exact file
resolved="$(resolve_import "com.example.app.models.User" "src/main/kotlin/com/example/app/Main.kt")"
assert_eq "specific import resolves to file" "src/main/kotlin/com/example/app/models/User.kt" "$resolved"

# Wildcard import resolves to all .kt files in package dir
wildcard="$(resolve_import "com.example.app.models.*" "src/main/kotlin/com/example/app/Main.kt")"
assert_contains "wildcard resolves User.kt" "src/main/kotlin/com/example/app/models/User.kt" "$wildcard"
assert_contains "wildcard resolves Order.kt" "src/main/kotlin/com/example/app/models/Order.kt" "$wildcard"
assert_line_count "wildcard yields 2 files" 2 "$wildcard"

# External package (not under PACKAGE_NAME) skipped
external="$(resolve_import "com.facebook.react.ReactPackage" "src/main/kotlin/com/example/app/Main.kt")"
assert_eq "external package skipped when PACKAGE_NAME set" "" "$external"

# Aliased import strips alias before resolving
aliased="$(resolve_import "com.example.app.models.User as U" "src/main/kotlin/com/example/app/Main.kt")"
assert_eq "aliased import resolves after stripping alias" "src/main/kotlin/com/example/app/models/User.kt" "$aliased"

report
