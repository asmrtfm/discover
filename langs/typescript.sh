#!/usr/bin/env bash
# Language config: TypeScript

LANG_ID="typescript"
FILE_EXT=".ts .tsx"
FILE_EXCLUDE_GLOBS="*.d.ts *.spec.ts *.test.ts *.spec.tsx *.test.tsx"
SRC_DIR_REL="src"
ENTRY_POINT_REL=""  # set per-project, e.g. "src/index.ts"
PACKAGE_NAME=""     # set per-project from package.json "name" field
CTX7_LIBRARIES="typescript node.js"
LSP_PLUGIN="typescript-lsp@claude-plugins-official"
LSP_BINARY="typescript-language-server"
LSP_INSTALL="npm install -g typescript-language-server typescript"

# ─────────────────────────────────────────────────────────
# Skill metadata — used by `discover init` to fill SKILL.md.template
# ─────────────────────────────────────────────────────────
SKILL_LANG_NAME="TypeScript"
SKILL_IMPORT_VERB="import"
SKILL_MOTIVATION="TypeScript's import resolution involves path aliases (\`@/\`, \`~/\`), barrel re-exports, and extensionless specifiers that resolve through tsconfig \`paths\` and Node module resolution — grep can't distinguish local modules from node_modules or tell you if a barrel re-export chain actually reaches a definition. discover.sh resolves the real import graph."
SKILL_RESOLVE_EXAMPLE_1="./services/auth"
SKILL_RESOLVE_RESULT_1="src/services/auth.ts"
SKILL_RESOLVE_EXAMPLE_2="../utils/helpers"
SKILL_RESOLVE_FROM_2="src/routes/index.ts"
SKILL_RESOLVE_RESULT_2="src/utils/helpers.ts"
SKILL_EXAMPLE_FILE="src/index.ts"
SKILL_EXAMPLE_SYMBOL="App"
SKILL_USAGE_EXAMPLE_1='new $CLASS($$$ARGS)'
SKILL_USAGE_EXAMPLE_2='async function $NAME($$$PARAMS): Promise<$RET>'

# ─────────────────────────────────────────────────────────
# ast-grep language selection: .tsx files need -l tsx
# ─────────────────────────────────────────────────────────
_ts_lang() {
    case "$1" in
        *.tsx) echo "tsx" ;;
        *)     echo "typescript" ;;
    esac
}

ast_grep_languages() { echo "typescript tsx"; }

# ─────────────────────────────────────────────────────────
# TypeScript has import and export-from directives
# ─────────────────────────────────────────────────────────
import_directives() { echo "import export"; }

inspect_sections() { echo "imports exports entities"; }

# ─────────────────────────────────────────────────────────
# Import resolution
#
# TypeScript import forms:
#   import { X } from "./foo"       → relative, extensionless
#   import { X } from "./foo.js"    → relative with extension
#   import * as X from "../bar"     → relative namespace import
#   import X from "package"         → node_modules, unresolvable
#   import type { X } from "./foo"  → type-only, still a dependency
#   export { X } from "./foo"       → re-export, same resolution
#   export * from "./foo"           → barrel re-export
# ─────────────────────────────────────────────────────────
resolve_import() {
    local specifier="$1" from_file="$2"

    # Skip bare module specifiers (node_modules / external packages)
    case "$specifier" in
        ./*|../*) ;;
        *) return ;;
    esac

    [ -n "$from_file" ] || return

    local dir
    dir="$(dirname "$(realpath "$REPO_ROOT/$from_file" 2>/dev/null)")"

    # Strip .js/.mjs/.cjs extension if present (TypeScript emits .js in specifiers)
    local base="${specifier%.js}"
    base="${base%.mjs}"
    base="${base%.cjs}"

    # Try .ts, .tsx, then /index.ts, /index.tsx
    local candidate
    for ext in .ts .tsx; do
        candidate="$(realpath -m "$dir/$base$ext" 2>/dev/null)"
        [ -f "$candidate" ] && { realpath --relative-to="$REPO_ROOT" "$candidate" 2>/dev/null; return; }
    done

    # Directory with index file
    local dir_candidate
    dir_candidate="$(realpath -m "$dir/$base" 2>/dev/null)"
    if [ -d "$dir_candidate" ]; then
        for ext in .ts .tsx; do
            [ -f "$dir_candidate/index$ext" ] && { realpath --relative-to="$REPO_ROOT" "$dir_candidate/index$ext" 2>/dev/null; return; }
        done
    fi

    # Exact path (already has .ts/.tsx extension)
    candidate="$(realpath -m "$dir/$specifier" 2>/dev/null)"
    [ -f "$candidate" ] && { realpath --relative-to="$REPO_ROOT" "$candidate" 2>/dev/null; return; }
}

# ─────────────────────────────────────────────────────────
# ast-grep patterns for TypeScript imports and exports
# ─────────────────────────────────────────────────────────
ast_grep_imports() {
    ast_grep_directive_imports "$1" "import"
}

ast_grep_directive_imports() {
    local file="$1" directive="$2"
    local lang
    lang="$(_ts_lang "$file")"
    case "$directive" in
        import)
            ast-grep run -l "$lang" -p 'import $$$SPEC from "$URI"' "$file" --json 2>/dev/null \
                | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
            ;;
        export)
            {
                ast-grep run -l "$lang" -p 'export { $$$NAMES } from "$URI"' "$file" --json 2>/dev/null \
                    | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
                ast-grep run -l "$lang" -p 'export * from "$URI"' "$file" --json 2>/dev/null \
                    | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
                ast-grep run -l "$lang" -p 'export type { $$$NAMES } from "$URI"' "$file" --json 2>/dev/null \
                    | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
            } | sort -u
            ;;
    esac
}

# ─────────────────────────────────────────────────────────
# Entity extraction
# ─────────────────────────────────────────────────────────
ast_grep_entities() {
    local file="$1" depth="${2:-names}"
    local lang
    lang="$(_ts_lang "$file")"

    # AST node kinds → display keyword
    local kinds="interface_declaration type_alias_declaration enum_declaration class_declaration abstract_class_declaration function_declaration"
    local keyword_map="interface_declaration:interface type_alias_declaration:type enum_declaration:enum class_declaration:class abstract_class_declaration:abstract_class function_declaration:function"

    local all_json="[]"

    for kind in $kinds; do
        local keyword=""
        for pair in $keyword_map; do
            [ "${pair%%:*}" = "$kind" ] && { keyword="${pair#*:}"; break; }
        done

        local raw
        raw="$(ast-grep run -l "$lang" --kind "$kind" "$file" --json 2>/dev/null)"
        [ -n "$raw" ] && [ "$raw" != "[]" ] || continue

        # Name capture pattern varies by kind
        local name_pattern
        case "$kind" in
            abstract_class_declaration) name_pattern="class\\s+(?<n>\\w+)" ;;
            type_alias_declaration)     name_pattern="type\\s+(?<n>\\w+)" ;;
            *)                          name_pattern="${keyword}\\s+(?<n>\\w+)" ;;
        esac

        local extracted
        extracted="$(echo "$raw" | jq --arg kw "$keyword" --arg depth "$depth" --arg np "$name_pattern" '
            [.[] | {
                kind: $kw,
                name: (.text | capture($np) | .n // "?"),
                line: (.range.start.line + 1),
                text: (
                    if $depth == "full" then .text
                    elif $depth == "signatures" then (.text | split("\n") | map(select(test($np)))[0] // (.text | split("\n")[0]))
                    else null
                    end
                )
            }]
        ' 2>/dev/null)"

        [ -n "$extracted" ] && [ "$extracted" != "[]" ] || continue
        all_json="$(echo "$all_json" "$extracted" | jq -s '.[0] + .[1]')"
    done

    # Top-level const/let declarations (lexical_declaration at column 0)
    local lex_raw
    lex_raw="$(ast-grep run -l "$lang" --kind lexical_declaration "$file" --json 2>/dev/null)"
    if [ -n "$lex_raw" ] && [ "$lex_raw" != "[]" ]; then
        local lex_extracted
        lex_extracted="$(echo "$lex_raw" | jq --arg depth "$depth" '
            [.[] | select(.range.start.column == 0 or (.text | test("^export"))) | {
                kind: (.text | split("\n")[0] | (if test("^(export\\s+)?const") then "const" else "let" end)),
                name: (.text | split("\n")[0] | capture("(?:const|let)\\s+(?<n>\\w+)") | .n // "?"),
                line: (.range.start.line + 1),
                text: (
                    if $depth == "full" then .text
                    elif $depth == "signatures" then (.text | split("\n")[0])
                    else null
                    end
                )
            }]
        ' 2>/dev/null)"

        if [ -n "$lex_extracted" ] && [ "$lex_extracted" != "[]" ]; then
            all_json="$(echo "$all_json" "$lex_extracted" | jq -s '.[0] + .[1]')"
        fi
    fi

    # Namespaces (internal_module)
    local ns_raw
    ns_raw="$(ast-grep run -l "$lang" --kind internal_module "$file" --json 2>/dev/null)"
    if [ -n "$ns_raw" ] && [ "$ns_raw" != "[]" ]; then
        local ns_extracted
        ns_extracted="$(echo "$ns_raw" | jq --arg depth "$depth" '
            [.[] | {
                kind: "namespace",
                name: (.text | split("\n")[0] | capture("(?:namespace|module)\\s+(?<n>\\w+)") | .n // "?"),
                line: (.range.start.line + 1),
                text: (
                    if $depth == "full" then .text
                    elif $depth == "signatures" then (.text | split("\n")[0])
                    else null
                    end
                )
            }]
        ' 2>/dev/null)"

        if [ -n "$ns_extracted" ] && [ "$ns_extracted" != "[]" ]; then
            all_json="$(echo "$all_json" "$ns_extracted" | jq -s '.[0] + .[1]')"
        fi
    fi

    echo "$all_json" | jq 'sort_by(.line) | if .[0].text == null then [.[] | del(.text)] else . end'
}
