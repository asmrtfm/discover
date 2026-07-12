#!/usr/bin/env bash
# discover-react.sh — React-specific structural analysis that complements discover.sh.
# Uses ast-grep for static AST queries against JSX/TSX component patterns.
#
# Terminology:
#   components  — React function or class components (files exporting JSX)
#   hooks       — custom hooks (useXxx functions) and their call sites
#   contexts    — createContext/useContext provider-consumer relationships
#   routes      — route definitions from react-router or Next.js conventions

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(realpath "$SCRIPT_DIR/../..")"
DISCOVER_SH="$SCRIPT_DIR/discover.sh"

die() { echo "error: $*" >&2; exit 1; }

# Resolve ast-grep language from file extension
_lang_for() {
    case "$1" in
        *.tsx|*.jsx) echo "tsx" ;;
        *)           echo "typescript" ;;
    esac
}

# Find all TS/TSX/JS/JSX source files under SRC_DIR
_src_files() {
    local src_dir="${1:-src}"
    find "$REPO_ROOT/$src_dir" -type f \( -name "*.tsx" -o -name "*.jsx" -o -name "*.ts" -o -name "*.js" \) \
        ! -name "*.d.ts" ! -name "*.spec.*" ! -name "*.test.*" ! -path "*/node_modules/*" \
        2>/dev/null | sort
}

# ─────────────────────────────────────────────────────────
# Subcommand: components
# Find React components — functions that return JSX.
# ─────────────────────────────────────────────────────────
cmd_components() {
    local target="${1:-}" format="text" src_dir="src"

    while [ $# -gt 0 ]; do
        case "$1" in
            --json) format="json" ;;
            --src-dir) src_dir="$2"; shift ;;
            *) target="$1" ;;
        esac
        shift
    done

    if [ -n "$target" ] && [ -f "$REPO_ROOT/$target" ]; then
        # Inspect a specific file for component definitions
        _components_in_file "$target" "$format"
        return
    fi

    # Scan all source files for exported components
    local results="[]"
    while IFS= read -r file; do
        local rel
        rel="$(realpath --relative-to="$REPO_ROOT" "$file")"
        local lang
        lang="$(_lang_for "$file")"

        # Look for JSX return patterns — function components returning JSX
        local matches
        matches="$(ast-grep run -l "$lang" --kind jsx_element "$file" --json 2>/dev/null)"
        [ -n "$matches" ] && [ "$matches" != "[]" ] || continue

        # Extract component names from the file
        local components
        components="$(_extract_component_names "$file" "$lang")"
        [ -n "$components" ] && [ "$components" != "[]" ] || continue

        if [ -n "$target" ]; then
            # Filter to matching component name
            components="$(echo "$components" | jq --arg name "$target" '[.[] | select(.name == $name)]')"
            [ "$components" != "[]" ] || continue
        fi

        results="$(echo "$results" "$components" | jq -s --arg file "$rel" '
            .[0] + [.[1][] | . + {file: $file}]
        ')"
    done < <(_src_files "$src_dir")

    if [ "$format" = "json" ]; then
        echo "$results" | jq '.'
    else
        echo "$results" | jq -r '.[] | "\(.name)\t\(.file):\(.line)"'
    fi
}

_extract_component_names() {
    local file="$1" lang="$2"
    local results="[]"

    # Function declarations
    local fn_matches
    fn_matches="$(ast-grep run -l "$lang" --kind function_declaration "$file" --json 2>/dev/null)"
    if [ -n "$fn_matches" ] && [ "$fn_matches" != "[]" ]; then
        local fn_components
        fn_components="$(echo "$fn_matches" | jq '
            [.[] | select(.text | test("return[\\s\\S]*<")) |
            {
                name: (.text | capture("function\\s+(?<n>[A-Z]\\w*)") | .n // null),
                kind: "function",
                line: (.range.start.line + 1)
            } | select(.name != null)]
        ' 2>/dev/null)"
        [ -n "$fn_components" ] && [ "$fn_components" != "[]" ] && \
            results="$(echo "$results" "$fn_components" | jq -s '.[0] + .[1]')"
    fi

    # Arrow function components: const Foo = (...) => ...
    local lex_matches
    lex_matches="$(ast-grep run -l "$lang" --kind lexical_declaration "$file" --json 2>/dev/null)"
    if [ -n "$lex_matches" ] && [ "$lex_matches" != "[]" ]; then
        local arrow_components
        arrow_components="$(echo "$lex_matches" | jq '
            [.[] | select(.text | test("=>")) | select(.text | test("<|jsx|React\\.createElement")) |
            {
                name: (.text | capture("(?:const|let)\\s+(?<n>[A-Z]\\w*)") | .n // null),
                kind: "arrow",
                line: (.range.start.line + 1)
            } | select(.name != null)]
        ' 2>/dev/null)"
        [ -n "$arrow_components" ] && [ "$arrow_components" != "[]" ] && \
            results="$(echo "$results" "$arrow_components" | jq -s '.[0] + .[1]')"
    fi

    echo "$results" | jq 'unique_by(.name) | sort_by(.line)'
}

_components_in_file() {
    local file="$1" format="$2"
    local lang
    lang="$(_lang_for "$file")"
    local components
    components="$(_extract_component_names "$REPO_ROOT/$file" "$lang")"

    if [ "$format" = "json" ]; then
        echo "$components" | jq --arg file "$file" '[.[] | . + {file: $file}]'
    else
        echo "$components" | jq -r --arg file "$file" '.[] | "\(.name)\t\($file):\(.line)"'
    fi
}

# ─────────────────────────────────────────────────────────
# Subcommand: hooks
# Find custom hook definitions (useXxx) and their usage sites.
# ─────────────────────────────────────────────────────────
cmd_hooks() {
    local target="${1:-}" format="text" src_dir="src"

    while [ $# -gt 0 ]; do
        case "$1" in
            --json) format="json" ;;
            --src-dir) src_dir="$2"; shift ;;
            *) target="$1" ;;
        esac
        shift
    done

    if [ -n "$target" ]; then
        # Find usages of a specific hook
        _hook_usages "$target" "$src_dir" "$format"
        return
    fi

    # Find all custom hook definitions
    local results="[]"
    while IFS= read -r file; do
        local rel
        rel="$(realpath --relative-to="$REPO_ROOT" "$file")"
        local lang
        lang="$(_lang_for "$file")"

        # Function declarations: function useXxx(...)
        local fn_hooks
        fn_hooks="$(ast-grep run -l "$lang" -p 'function $HOOK($$$PARAMS) { $$$BODY }' "$file" --json 2>/dev/null \
            | jq '[.[] | select(.metaVariables.single.HOOK.text | test("^use[A-Z]")) | {
                name: .metaVariables.single.HOOK.text,
                kind: "function",
                line: (.range.start.line + 1)
            }]' 2>/dev/null)"
        [ -n "$fn_hooks" ] && [ "$fn_hooks" != "[]" ] && \
            results="$(echo "$results" "$fn_hooks" | jq -s --arg file "$rel" '.[0] + [.[1][] | . + {file: $file}]')"

        # Arrow function hooks: const useXxx = ...
        local arrow_hooks
        arrow_hooks="$(ast-grep run -l "$lang" --kind lexical_declaration "$file" --json 2>/dev/null \
            | jq '[.[] | select(.text | test("(?:const|let|export)\\s+use[A-Z]")) | {
                name: (.text | capture("(?:const|let)\\s+(?<n>use[A-Z]\\w*)") | .n // null),
                kind: "arrow",
                line: (.range.start.line + 1)
            } | select(.name != null)]' 2>/dev/null)"
        [ -n "$arrow_hooks" ] && [ "$arrow_hooks" != "[]" ] && \
            results="$(echo "$results" "$arrow_hooks" | jq -s --arg file "$rel" '.[0] + [.[1][] | . + {file: $file}]')"
    done < <(_src_files "$src_dir")

    if [ "$format" = "json" ]; then
        echo "$results" | jq 'unique_by(.name) | sort_by(.name)'
    else
        echo "$results" | jq -r 'unique_by(.name) | sort_by(.name) | .[] | "\(.name)\t\(.file):\(.line)"'
    fi
}

_hook_usages() {
    local hook="$1" src_dir="$2" format="$3"
    local results="[]"

    while IFS= read -r file; do
        local rel
        rel="$(realpath --relative-to="$REPO_ROOT" "$file")"
        local lang
        lang="$(_lang_for "$file")"

        local matches
        matches="$(ast-grep run -l "$lang" -p "${hook}($$$ARGS)" "$file" --json 2>/dev/null)"
        [ -n "$matches" ] && [ "$matches" != "[]" ] || continue

        local usages
        usages="$(echo "$matches" | jq --arg file "$rel" '[.[] | {
            file: $file,
            line: (.range.start.line + 1),
            text: (.text | split("\n")[0])
        }]' 2>/dev/null)"

        [ -n "$usages" ] && [ "$usages" != "[]" ] && \
            results="$(echo "$results" "$usages" | jq -s '.[0] + .[1]')"
    done < <(_src_files "$src_dir")

    if [ "$format" = "json" ]; then
        echo "$results" | jq '.'
    else
        echo "$results" | jq -r '.[] | "\(.file):\(.line)\t\(.text)"'
    fi
}

# ─────────────────────────────────────────────────────────
# Subcommand: contexts
# Map createContext/useContext provider-consumer relationships.
# ─────────────────────────────────────────────────────────
cmd_contexts() {
    local target="${1:-}" format="text" src_dir="src"

    while [ $# -gt 0 ]; do
        case "$1" in
            --json) format="json" ;;
            --src-dir) src_dir="$2"; shift ;;
            *) target="$1" ;;
        esac
        shift
    done

    local definitions="[]"
    local providers="[]"
    local consumers="[]"

    while IFS= read -r file; do
        local rel
        rel="$(realpath --relative-to="$REPO_ROOT" "$file")"
        local lang
        lang="$(_lang_for "$file")"

        # createContext definitions
        local ctx_defs
        ctx_defs="$(ast-grep run -l "$lang" -p 'createContext($$$ARGS)' "$file" --json 2>/dev/null)"
        if [ -n "$ctx_defs" ] && [ "$ctx_defs" != "[]" ]; then
            local defs
            defs="$(echo "$ctx_defs" | jq --arg file "$rel" '[.[] | {
                name: (input_line_number // null),
                file: $file,
                line: (.range.start.line + 1),
                text: (.text | split("\n")[0])
            }]' 2>/dev/null)"

            # Try to extract the variable name from surrounding context
            local lex_raw
            lex_raw="$(ast-grep run -l "$lang" --kind lexical_declaration "$file" --json 2>/dev/null)"
            if [ -n "$lex_raw" ] && [ "$lex_raw" != "[]" ]; then
                local named_defs
                named_defs="$(echo "$lex_raw" | jq --arg file "$rel" '[.[] | select(.text | test("createContext")) | {
                    name: (.text | capture("(?:const|let|export)\\s+(?:const\\s+)?(?<n>\\w+)") | .n // "anonymous"),
                    file: $file,
                    line: (.range.start.line + 1),
                    text: (.text | split("\n")[0])
                }]' 2>/dev/null)"
                [ -n "$named_defs" ] && [ "$named_defs" != "[]" ] && \
                    definitions="$(echo "$definitions" "$named_defs" | jq -s '.[0] + .[1]')"
            fi
        fi

        # Context.Provider usage in JSX
        local prov_matches
        prov_matches="$(ast-grep run -l "$lang" -p '$CTX.Provider' "$file" --json 2>/dev/null)"
        if [ -n "$prov_matches" ] && [ "$prov_matches" != "[]" ]; then
            local provs
            provs="$(echo "$prov_matches" | jq --arg file "$rel" '[.[] | {
                context: (.text | capture("(?<n>\\w+)\\.Provider") | .n // "?"),
                file: $file,
                line: (.range.start.line + 1)
            }]' 2>/dev/null)"
            [ -n "$provs" ] && [ "$provs" != "[]" ] && \
                providers="$(echo "$providers" "$provs" | jq -s '.[0] + .[1]')"
        fi

        # useContext consumers
        local cons_matches
        cons_matches="$(ast-grep run -l "$lang" -p 'useContext($CTX)' "$file" --json 2>/dev/null)"
        if [ -n "$cons_matches" ] && [ "$cons_matches" != "[]" ]; then
            local cons
            cons="$(echo "$cons_matches" | jq --arg file "$rel" '[.[] | {
                context: .metaVariables.single.CTX.text,
                file: $file,
                line: (.range.start.line + 1)
            }]' 2>/dev/null)"
            [ -n "$cons" ] && [ "$cons" != "[]" ] && \
                consumers="$(echo "$consumers" "$cons" | jq -s '.[0] + .[1]')"
        fi
    done < <(_src_files "$src_dir")

    # Filter to target context if specified
    if [ -n "$target" ]; then
        definitions="$(echo "$definitions" | jq --arg t "$target" '[.[] | select(.name == $t)]')"
        providers="$(echo "$providers" | jq --arg t "$target" '[.[] | select(.context == $t)]')"
        consumers="$(echo "$consumers" | jq --arg t "$target" '[.[] | select(.context == $t)]')"
    fi

    if [ "$format" = "json" ]; then
        jq -n --argjson defs "$definitions" --argjson provs "$providers" --argjson cons "$consumers" \
            '{definitions: $defs, providers: $provs, consumers: $cons}'
    else
        echo "=== Context Definitions ==="
        echo "$definitions" | jq -r '.[] | "  \(.name)\t\(.file):\(.line)"'
        echo ""
        echo "=== Providers ==="
        echo "$providers" | jq -r '.[] | "  \(.context)\t\(.file):\(.line)"'
        echo ""
        echo "=== Consumers ==="
        echo "$consumers" | jq -r '.[] | "  \(.context)\t\(.file):\(.line)"'
    fi
}

# ─────────────────────────────────────────────────────────
# Subcommand: routes
# Extract route definitions from react-router declarative config.
# ─────────────────────────────────────────────────────────
cmd_routes() {
    local format="text" src_dir="src"

    while [ $# -gt 0 ]; do
        case "$1" in
            --json) format="json" ;;
            --src-dir) src_dir="$2"; shift ;;
        esac
        shift
    done

    local results="[]"

    while IFS= read -r file; do
        local rel
        rel="$(realpath --relative-to="$REPO_ROOT" "$file")"
        local lang
        lang="$(_lang_for "$file")"

        # <Route path="..." .../> patterns
        local route_matches
        route_matches="$(ast-grep run -l "$lang" -p '<Route path="$PATH" $$$ATTRS />' "$file" --json 2>/dev/null)"
        if [ -n "$route_matches" ] && [ "$route_matches" != "[]" ]; then
            local routes
            routes="$(echo "$route_matches" | jq --arg file "$rel" '[.[] | {
                path: .metaVariables.single.PATH.text,
                file: $file,
                line: (.range.start.line + 1),
                text: (.text | split("\n")[0])
            }]' 2>/dev/null)"
            [ -n "$routes" ] && [ "$routes" != "[]" ] && \
                results="$(echo "$results" "$routes" | jq -s '.[0] + .[1]')"
        fi

        # <Route path="...">...</Route> patterns (with children)
        local route_matches2
        route_matches2="$(ast-grep run -l "$lang" -p '<Route path="$PATH" $$$ATTRS>$$$CHILDREN</Route>' "$file" --json 2>/dev/null)"
        if [ -n "$route_matches2" ] && [ "$route_matches2" != "[]" ]; then
            local routes2
            routes2="$(echo "$route_matches2" | jq --arg file "$rel" '[.[] | {
                path: .metaVariables.single.PATH.text,
                file: $file,
                line: (.range.start.line + 1),
                text: (.text | split("\n")[0])
            }]' 2>/dev/null)"
            [ -n "$routes2" ] && [ "$routes2" != "[]" ] && \
                results="$(echo "$results" "$routes2" | jq -s '.[0] + .[1]')"
        fi
    done < <(_src_files "$src_dir")

    # Deduplicate by path+file
    results="$(echo "$results" | jq 'unique_by(.path + .file) | sort_by(.path)')"

    if [ "$format" = "json" ]; then
        echo "$results" | jq '.'
    else
        echo "$results" | jq -r '.[] | "\(.path)\t\(.file):\(.line)"'
    fi
}

# ─────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────
main() {
    local subcmd="${1:-}"
    shift 2>/dev/null || true

    case "$subcmd" in
        components)  cmd_components "$@" ;;
        hooks)       cmd_hooks "$@" ;;
        contexts)    cmd_contexts "$@" ;;
        routes)      cmd_routes "$@" ;;
        *)
            echo "usage: discover-react.sh <command> [args]" >&2
            echo "" >&2
            echo "commands:" >&2
            echo "  components [NAME|FILE] [--json]    Find React components (functions returning JSX)" >&2
            echo "  hooks [HOOK_NAME] [--json]         Find custom hook definitions or usages of a hook" >&2
            echo "  contexts [CONTEXT] [--json]        Map createContext/useContext provider-consumer graph" >&2
            echo "  routes [--json]                    Extract route definitions (react-router)" >&2
            exit 1
            ;;
    esac
}

main "$@"
