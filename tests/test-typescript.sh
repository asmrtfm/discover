#!/usr/bin/env bash
# test-typescript.sh — discover framework tests for TypeScript lang config
source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"
load_lang typescript

FIXTURE="$TESTS_DIR/fixtures/typescript.ts"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_imports ==="

imports="$(ast_grep_imports "$FIXTURE")"

assert_contains "extracts named import" "react" "$imports"
assert_contains "extracts namespace import" "fs" "$imports"
assert_contains "extracts default import" "path" "$imports"
assert_contains "extracts type import" "./config" "$imports"
assert_contains "extracts mixed import" "./users" "$imports"
assert_line_count "5 total imports" 5 "$imports"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_directive_imports (export) ==="

exports="$(ast_grep_directive_imports "$FIXTURE" export)"

assert_contains "extracts named re-export" "./helpers" "$exports"
assert_contains "extracts star re-export" "./utils" "$exports"
assert_contains "extracts type re-export" "./options" "$exports"
assert_line_count "3 total exports" 3 "$exports"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (names) ==="

entities="$(ast_grep_entities "$FIXTURE" names)"

assert_contains "finds MyNamespace" '"name": "MyNamespace"' "$entities"
assert_contains "finds Greeter interface" '"name": "Greeter"' "$entities"
assert_contains "finds Result type" '"name": "Result"' "$entities"
assert_contains "finds Direction enum" '"name": "Direction"' "$entities"
assert_contains "finds Animal class" '"name": "Animal"' "$entities"
assert_contains "finds Shape abstract class" '"name": "Shape"' "$entities"
assert_contains "finds greet function" '"name": "greet"' "$entities"
assert_contains "finds fetchData function" '"name": "fetchData"' "$entities"
assert_contains "finds add const" '"name": "add"' "$entities"
assert_contains "finds MAX_RETRIES const" '"name": "MAX_RETRIES"' "$entities"
assert_contains "finds currentRetry let" '"name": "currentRetry"' "$entities"
assert_contains "finds BugReport class" '"name": "BugReport"' "$entities"
assert_contains "finds identity function" '"name": "identity"' "$entities"
assert_contains "finds App class" '"name": "App"' "$entities"

entity_count="$(echo "$entities" | jq 'length')"
assert_eq "16 total entities" "16" "$entity_count"

# ─────────────────────────────────────────────────────────
echo "=== entity kinds ==="

assert_contains "namespace kind" '"kind": "namespace"' "$entities"
assert_contains "interface kind" '"kind": "interface"' "$entities"
assert_contains "type kind" '"kind": "type"' "$entities"
assert_contains "enum kind" '"kind": "enum"' "$entities"
assert_contains "class kind" '"kind": "class"' "$entities"
assert_contains "abstract_class kind" '"kind": "abstract_class"' "$entities"
assert_contains "function kind" '"kind": "function"' "$entities"
assert_contains "const kind" '"kind": "const"' "$entities"
assert_contains "let kind" '"kind": "let"' "$entities"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_entities (signatures) ==="

sigs="$(ast_grep_entities "$FIXTURE" signatures)"

assert_contains "abstract class signature" \
    '"text": "abstract class Shape {"' "$sigs"
assert_contains "async function signature" \
    '"text": "async function fetchData(url: string): Promise<Response> {"' "$sigs"
assert_contains "generic function signature" \
    '"text": "function identity<T>(arg: T): T {"' "$sigs"
assert_contains "arrow const signature" \
    '"text": "const add = (a: number, b: number): number => a + b;"' "$sigs"
assert_contains "type alias signature" \
    '"text": "type Result<T> = { ok: true; value: T } | { ok: false; error: Error };"' "$sigs"

# ─────────────────────────────────────────────────────────
echo "=== no inner method leak ==="

assert_not_contains "speak() not extracted" '"name": "speak"' "$entities"
assert_not_contains "getCount() not extracted" '"name": "getCount"' "$entities"
assert_not_contains "constructor not extracted" '"name": "constructor"' "$entities"

# ─────────────────────────────────────────────────────────
# TSX fixture — verifies .tsx files parse with -l tsx
# ─────────────────────────────────────────────────────────
TSX_FIXTURE="$TESTS_DIR/fixtures/typescript.tsx"

echo "=== tsx: ast_grep_imports ==="

tsx_imports="$(ast_grep_imports "$TSX_FIXTURE")"

assert_contains "tsx extracts react import" "react" "$tsx_imports"
assert_contains "tsx extracts react-native import" "react-native" "$tsx_imports"
assert_contains "tsx extracts relative import" "./hooks/useTheme" "$tsx_imports"
assert_line_count "tsx 4 total imports" 4 "$tsx_imports"

# ─────────────────────────────────────────────────────────
echo "=== tsx: ast_grep_entities (names) ==="

tsx_entities="$(ast_grep_entities "$TSX_FIXTURE" names)"

assert_contains "tsx finds CounterProps interface" '"name": "CounterProps"' "$tsx_entities"
assert_contains "tsx finds CounterMode enum" '"name": "CounterMode"' "$tsx_entities"
assert_contains "tsx finds Counter const" '"name": "Counter"' "$tsx_entities"
assert_contains "tsx finds styles const" '"name": "styles"' "$tsx_entities"
assert_contains "tsx finds withLogging function" '"name": "withLogging"' "$tsx_entities"
assert_contains "tsx finds useCounter function" '"name": "useCounter"' "$tsx_entities"

tsx_entity_count="$(echo "$tsx_entities" | jq 'length')"
assert_eq "tsx 6 total entities" "6" "$tsx_entity_count"

# ─────────────────────────────────────────────────────────
echo "=== tsx: no inner hook leak ==="

assert_not_contains "tsx useState not extracted" '"name": "useState"' "$tsx_entities"
assert_not_contains "tsx useEffect not extracted" '"name": "useEffect"' "$tsx_entities"

# ─────────────────────────────────────────────────────────
echo "=== tsx: _ts_lang dispatch ==="

assert_eq "_ts_lang picks tsx for .tsx" "tsx" "$(_ts_lang "src/App.tsx")"
assert_eq "_ts_lang picks typescript for .ts" "typescript" "$(_ts_lang "src/index.ts")"

# ─────────────────────────────────────────────────────────
echo "=== ast_grep_languages ==="

ag_langs="$(ast_grep_languages)"
assert_contains "ast_grep_languages includes typescript" "typescript" "$ag_langs"
assert_contains "ast_grep_languages includes tsx" "tsx" "$ag_langs"

report
