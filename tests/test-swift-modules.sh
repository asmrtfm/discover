#!/usr/bin/env bash
# test-swift-modules.sh -- tests for Swift module-aware navigation
#
# Tests P0-P3: single-target fallback, module graph, definition across
# modules, importers, live-files, modules/module-of subcommands.
source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"

FIXTURE_PROJECT="$TESTS_DIR/fixtures/swift-project"

# Source once with REPO_ROOT pointing at fixture project
export REPO_ROOT="$FIXTURE_PROJECT"
export SCRIPT_DIR="$FIXTURE_PROJECT"
source "$DISCOVER_ROOT/langs/swift.sh"
source "$DISCOVER_ROOT/lib/core.sh"

# -----------------------------------------------------------
echo "=== single-target fallback ==="
# Reset and force single-target mode
_SWIFT_MODULES_LOADED=false
_SWIFT_MODULE_DIRS=()
_SWIFT_MODULE_DEPS=()
_SWIFT_MODULE_TYPES=()
_swift_load_single_target

mod="$(_swift_app_module)"
assert_eq "app module found" "app" "$mod"
assert_eq "project type" "single" "$_SWIFT_PROJECT_TYPE"

files="$(_swift_module_files "app")"
file_count="$(echo "$files" | grep -c . || true)"
# SRC_DIR_REL is "Sources" so it finds all .swift in Sources/ recursively
assert_eq "all fixture files found (5)" "5" "$file_count"
assert_contains "AppMain in file list" "Sources/App/AppMain.swift" "$files"
assert_contains "User in file list" "Sources/Models/User.swift" "$files"

# -----------------------------------------------------------
echo "=== module graph (manual setup) ==="
# Manually set up the module graph as if swift package dump-package succeeded
_SWIFT_MODULES_LOADED=true
_SWIFT_PROJECT_TYPE="spm"
_SWIFT_MODULE_DIRS=([App]="Sources/App" [NetworkKit]="Sources/NetworkKit" [Models]="Sources/Models" [AppTests]="Tests/AppTests")
_SWIFT_MODULE_DEPS=([App]="NetworkKit Models" [NetworkKit]="Models" [Models]="" [AppTests]="App")
_SWIFT_MODULE_TYPES=([App]="executable" [NetworkKit]="library" [Models]="library" [AppTests]="test")
# Re-derive SRC_DIR since we changed REPO_ROOT
SRC_DIR="$REPO_ROOT/$SRC_DIR_REL"

app_mod="$(_swift_app_module)"
assert_eq "app module is App" "App" "$app_mod"

# -----------------------------------------------------------
echo "=== module-of ==="

mod="$(_swift_module_of_file "Sources/App/AppMain.swift")"
assert_eq "AppMain belongs to App" "App" "$mod"

mod="$(_swift_module_of_file "Sources/NetworkKit/APIClient.swift")"
assert_eq "APIClient belongs to NetworkKit" "NetworkKit" "$mod"

mod="$(_swift_module_of_file "Sources/Models/User.swift")"
assert_eq "User belongs to Models" "Models" "$mod"

mod="$(_swift_module_of_file "Tests/AppTests/AppTests.swift")"
assert_eq "test file belongs to AppTests" "AppTests" "$mod"

# -----------------------------------------------------------
echo "=== reachable modules ==="

reachable="$(_swift_reachable_modules "App")"
assert_contains "App reachable from App" "App" "$reachable"
assert_contains "NetworkKit reachable from App" "NetworkKit" "$reachable"
assert_contains "Models reachable from App" "Models" "$reachable"
assert_not_contains "AppTests not reachable from App" "AppTests" "$reachable"

reachable="$(_swift_reachable_modules "NetworkKit")"
assert_contains "NetworkKit reachable from self" "NetworkKit" "$reachable"
assert_contains "Models reachable from NetworkKit" "Models" "$reachable"
assert_not_contains "App not reachable from NetworkKit" "App" "$reachable"

# -----------------------------------------------------------
echo "=== live-files (module-aware) ==="

live="$(build_live_files 2>/dev/null)"
assert_contains "App files are live" "Sources/App/AppMain.swift" "$live"
assert_contains "App NavigationManager is live" "Sources/App/NavigationManager.swift" "$live"
assert_contains "NetworkKit files are live" "Sources/NetworkKit/APIClient.swift" "$live"
assert_contains "Models files are live" "Sources/Models/User.swift" "$live"
assert_not_contains "test files not live" "Tests/AppTests" "$live"

live_count="$(echo "$live" | grep -c . || true)"
assert_eq "5 live files total" "5" "$live_count"

# -----------------------------------------------------------
echo "=== definition (same-module, no imports needed) ==="

# NavigationManager is in Sources/App/ but not imported by AppMain.swift
result="$(cmd_definition "Sources/App/AppMain.swift" "NavigationManager" 2>/dev/null)"
assert_contains "finds NavigationManager cross-file" '"NavigationManager"' "$result"
assert_contains "in same module" '"same-module"' "$result"
assert_contains "correct file" "Sources/App/NavigationManager.swift" "$result"

# -----------------------------------------------------------
echo "=== definition (cross-module, respects access control) ==="

# APIClient is public in NetworkKit -- should be found from App
result="$(cmd_definition "Sources/App/AppMain.swift" "APIClient" 2>/dev/null)"
assert_contains "finds public APIClient" '"APIClient"' "$result"
assert_contains "from imported module" "NetworkKit" "$result"

# User is public in Models -- found transitively
result="$(cmd_definition "Sources/App/AppMain.swift" "User" 2>/dev/null)"
assert_contains "finds public User" '"User"' "$result"
assert_contains "from Models" "Models" "$result"

# InternalHelper is internal in NetworkKit -- should NOT be found from App
result="$(cmd_definition "Sources/App/AppMain.swift" "InternalHelper" 2>/dev/null)"
assert_contains "internal symbol not found" '"error"' "$result"

# AuthEndpoint is public -- should be found
result="$(cmd_definition "Sources/App/AppMain.swift" "AuthEndpoint" 2>/dev/null)"
assert_contains "finds public AuthEndpoint" '"AuthEndpoint"' "$result"

# -----------------------------------------------------------
echo "=== importers (module-level) ==="

result="$(cmd_importers "Sources/NetworkKit/APIClient.swift" 2>/dev/null)"
assert_contains "App imports NetworkKit" '"App"' "$result"

result="$(cmd_importers "Sources/Models/User.swift" 2>/dev/null)"
assert_contains "App depends on Models" '"App"' "$result"
assert_contains "NetworkKit depends on Models" '"NetworkKit"' "$result"

# -----------------------------------------------------------
echo "=== importers (--symbol mode) ==="

result="$(cmd_importers "Sources/NetworkKit/APIClient.swift" --symbol APIClient 2>/dev/null)"
assert_contains "AppMain references APIClient" "Sources/App/AppMain.swift" "$result"

# -----------------------------------------------------------
echo "=== modules subcommand ==="

result="$(cmd_modules 2>/dev/null)"
assert_contains "lists App" "App (executable)" "$result"
assert_contains "lists NetworkKit" "NetworkKit (library)" "$result"
assert_contains "lists Models" "Models (library)" "$result"

result="$(cmd_modules --json 2>/dev/null)"
assert_contains "json has App" '"name": "App"' "$result"
assert_contains "json has dependencies" '"dependencies"' "$result"

# -----------------------------------------------------------
echo "=== module-of subcommand ==="

result="$(cmd_module_of "Sources/App/AppMain.swift" 2>/dev/null)"
assert_eq "module-of returns App" "App" "$result"

result="$(cmd_module_of "Sources/NetworkKit/Endpoints.swift" 2>/dev/null)"
assert_eq "module-of returns NetworkKit" "NetworkKit" "$result"

report
