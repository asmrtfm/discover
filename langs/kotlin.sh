#!/usr/bin/env bash
# Language config: Kotlin

LANG_ID="kotlin"
FILE_EXT=".kt"
FILE_EXCLUDE_GLOBS="*.kts"
SRC_DIR_REL="src/main/kotlin"
ENTRY_POINT_REL=""  # set per-project, e.g. "src/main/kotlin/com/example/app/Main.kt"
PACKAGE_NAME=""     # set per-project root package, e.g. "com.example.app"
CTX7_LIBRARIES="kotlin kotlinx"
LSP_PLUGIN="kotlin-lsp@claude-plugins-official"
LSP_BINARY="kotlin-language-server"
LSP_INSTALL=""  # installed via IDE or sdkman

# ─────────────────────────────────────────────────────────
# Skill metadata — used by `discover init` to fill SKILL.md.template
# ─────────────────────────────────────────────────────────
SKILL_LANG_NAME="Kotlin"
SKILL_IMPORT_VERB="import"
SKILL_MOTIVATION="Kotlin's package-based imports mean \`import com.example.Foo\` could resolve to any file declaring that class in its package — grep can't distinguish local modules from external dependencies or tell you which file actually provides a symbol. discover.sh resolves the import graph by mapping packages to source paths."
SKILL_RESOLVE_EXAMPLE_1="com.example.models.User"
SKILL_RESOLVE_RESULT_1="src/main/kotlin/com/example/models/User.kt"
SKILL_RESOLVE_EXAMPLE_2="com.example.utils.*"
SKILL_RESOLVE_FROM_2="src/main/kotlin/com/example/app/Main.kt"
SKILL_RESOLVE_RESULT_2="src/main/kotlin/com/example/utils/ (all .kt files)"
SKILL_EXAMPLE_FILE="src/main/kotlin/com/example/app/Main.kt"
SKILL_EXAMPLE_SYMBOL="AppConfig"
SKILL_USAGE_EXAMPLE_1='val $NAME = $CLASS($$$ARGS)'
SKILL_USAGE_EXAMPLE_2='suspend fun $NAME($$$PARAMS): $RET'

# ─────────────────────────────────────────────────────────
# Kotlin uses import only (no export/part directives)
# ─────────────────────────────────────────────────────────
import_directives() { echo "import"; }

inspect_sections() { echo "imports entities"; }

# ─────────────────────────────────────────────────────────
# Import resolution
#
# Kotlin import forms:
#   import com.example.Foo        — specific class/function
#   import com.example.*          — wildcard (all in package)
#   import com.example.Foo as Bar — aliased import
#   import kotlin.*               — stdlib, unresolvable
#   import android.*              — platform, unresolvable
#
# Resolution: convert dotted package path to filesystem path
# under SRC_DIR_REL. If a second source root exists (e.g.
# src/main/java for mixed Android projects), set
# KOTLIN_EXTRA_SRC_DIRS as a space-separated list.
# ─────────────────────────────────────────────────────────
resolve_import() {
    local specifier="$1" from_file="$2"

    # Strip alias suffix: "com.example.Foo as Bar" → "com.example.Foo"
    specifier="${specifier%% as *}"

    # Skip stdlib and platform imports
    case "$specifier" in
        kotlin.*|kotlinx.*|java.*|javax.*|android.*|androidx.*|org.jetbrains.*)
            return ;;
    esac

    # If PACKAGE_NAME is set, skip imports outside it (external deps)
    if [ -n "$PACKAGE_NAME" ]; then
        case "$specifier" in
            "${PACKAGE_NAME}".*) ;;
            *) return ;;
        esac
    fi

    local src_dirs="$REPO_ROOT/$SRC_DIR_REL"
    [ -n "${KOTLIN_EXTRA_SRC_DIRS:-}" ] && src_dirs="$src_dirs ${KOTLIN_EXTRA_SRC_DIRS}"

    # Convert dots to path separators
    local path_part="${specifier//./\/}"

    # Wildcard import: resolve to all .kt files in the directory
    if [[ "$specifier" == *'*' ]]; then
        local dir_part="${path_part%/\*}"
        for src_dir in $src_dirs; do
            local dir="$src_dir/$dir_part"
            if [ -d "$dir" ]; then
                find "$dir" -maxdepth 1 -name "*.kt" -type f 2>/dev/null | sort | while IFS= read -r f; do
                    realpath --relative-to="$REPO_ROOT" "$f" 2>/dev/null
                done
                return
            fi
        done
        return
    fi

    # Specific import: try as a file path
    for src_dir in $src_dirs; do
        local candidate="$src_dir/${path_part}.kt"
        if [ -f "$candidate" ]; then
            realpath --relative-to="$REPO_ROOT" "$candidate" 2>/dev/null
            return
        fi
    done

    # May refer to a nested class or top-level function — try parent package
    local parent_path="${path_part%/*}"
    local leaf="${specifier##*.}"
    for src_dir in $src_dirs; do
        if [ -d "$src_dir/$parent_path" ]; then
            # Search .kt files in the parent directory for the symbol
            find "$src_dir/$parent_path" -maxdepth 1 -name "*.kt" -type f 2>/dev/null | while IFS= read -r f; do
                if grep -qw "$leaf" "$f" 2>/dev/null; then
                    realpath --relative-to="$REPO_ROOT" "$f" 2>/dev/null
                    return
                fi
            done
        fi
    done
}

# ─────────────────────────────────────────────────────────
# ast-grep extraction for Kotlin imports
#
# Uses --kind import_header to preserve wildcard (*) and
# alias (as) syntax that -p 'import $URI' would lose.
# ─────────────────────────────────────────────────────────
ast_grep_imports() {
    local file="$1"
    ast-grep run -l kotlin --kind import_header "$file" --json 2>/dev/null \
        | jq -r '.[].text | ltrimstr("import ")' 2>/dev/null
}

ast_grep_directive_imports() {
    local file="$1" directive="$2"
    case "$directive" in
        import) ast_grep_imports "$file" ;;
    esac
}

# ─────────────────────────────────────────────────────────
# Entity extraction
#
# Kotlin's tree-sitter grammar maps several constructs to
# class_declaration (class, data class, enum class, sealed
# class, interface, abstract class, annotation class).
# object_declaration, function_declaration, type_alias, and
# property_declaration are separate kinds.
# ─────────────────────────────────────────────────────────
ast_grep_entities() {
    local file="$1" depth="${2:-names}"
    local all_json="[]"

    # --- class_declaration (top-level only) ---
    local class_raw
    class_raw="$(ast-grep run -l kotlin --kind class_declaration "$file" --json 2>/dev/null)"
    if [ -n "$class_raw" ] && [ "$class_raw" != "[]" ]; then
        local class_extracted
        class_extracted="$(echo "$class_raw" | jq --arg depth "$depth" '
            [.[] | select(.range.start.column == 0) |
                (.text | split("\n")[0]) as $first_line |
                ($first_line | capture("^(?<mod>enum|data|sealed|abstract|annotation)?\\s*(?<kw>class|interface)\\s+(?<n>\\w+)")) as $cap |
                select($cap != null) |
                {
                    kind: (
                        if $cap.mod == "enum" then "enum"
                        elif $cap.mod == "data" then "data_class"
                        elif $cap.mod == "sealed" then "sealed_class"
                        elif $cap.mod == "abstract" then "abstract_class"
                        elif $cap.mod == "annotation" then "annotation"
                        elif $cap.kw == "interface" then "interface"
                        else "class"
                        end
                    ),
                    name: $cap.n,
                    line: (.range.start.line + 1),
                    text: (
                        if $depth == "full" then .text
                        elif $depth == "signatures" then $first_line
                        else null
                        end
                    )
                }
            ]
        ' 2>/dev/null)"

        if [ -n "$class_extracted" ] && [ "$class_extracted" != "[]" ]; then
            all_json="$(echo "$all_json" "$class_extracted" | jq -s '.[0] + .[1]')"
        fi
    fi

    # --- object_declaration (top-level only, excludes companion) ---
    local obj_raw
    obj_raw="$(ast-grep run -l kotlin --kind object_declaration "$file" --json 2>/dev/null)"
    if [ -n "$obj_raw" ] && [ "$obj_raw" != "[]" ]; then
        local obj_extracted
        obj_extracted="$(echo "$obj_raw" | jq --arg depth "$depth" '
            [.[] | select(.range.start.column == 0) | {
                kind: "object",
                name: (.text | split("\n")[0] | capture("object\\s+(?<n>\\w+)") | .n // "?"),
                line: (.range.start.line + 1),
                text: (
                    if $depth == "full" then .text
                    elif $depth == "signatures" then (.text | split("\n")[0])
                    else null
                    end
                )
            }]
        ' 2>/dev/null)"

        if [ -n "$obj_extracted" ] && [ "$obj_extracted" != "[]" ]; then
            all_json="$(echo "$all_json" "$obj_extracted" | jq -s '.[0] + .[1]')"
        fi
    fi

    # --- function_declaration (top-level only, column 0) ---
    local fn_raw
    fn_raw="$(ast-grep run -l kotlin --kind function_declaration "$file" --json 2>/dev/null)"
    if [ -n "$fn_raw" ] && [ "$fn_raw" != "[]" ]; then
        local fn_extracted
        fn_extracted="$(echo "$fn_raw" | jq --arg depth "$depth" '
            [.[] | select(.range.start.column == 0) | {
                kind: "function",
                name: (.text | split("\n")[0] | capture("fun\\s+(?<n>\\w+)") | .n // "?"),
                line: (.range.start.line + 1),
                text: (
                    if $depth == "full" then .text
                    elif $depth == "signatures" then (.text | split("\n")[0])
                    else null
                    end
                )
            }]
        ' 2>/dev/null)"

        if [ -n "$fn_extracted" ] && [ "$fn_extracted" != "[]" ]; then
            all_json="$(echo "$all_json" "$fn_extracted" | jq -s '.[0] + .[1]')"
        fi
    fi

    # --- type_alias ---
    local ta_raw
    ta_raw="$(ast-grep run -l kotlin --kind type_alias "$file" --json 2>/dev/null)"
    if [ -n "$ta_raw" ] && [ "$ta_raw" != "[]" ]; then
        local ta_extracted
        ta_extracted="$(echo "$ta_raw" | jq --arg depth "$depth" '
            [.[] | select(.range.start.column == 0) | {
                kind: "typealias",
                name: (.text | split("\n")[0] | capture("typealias\\s+(?<n>\\w+)") | .n // "?"),
                line: (.range.start.line + 1),
                text: (
                    if $depth == "full" then .text
                    elif $depth == "signatures" then (.text | split("\n")[0])
                    else null
                    end
                )
            }]
        ' 2>/dev/null)"

        if [ -n "$ta_extracted" ] && [ "$ta_extracted" != "[]" ]; then
            all_json="$(echo "$all_json" "$ta_extracted" | jq -s '.[0] + .[1]')"
        fi
    fi

    # --- property_declaration (top-level only, column 0) ---
    local prop_raw
    prop_raw="$(ast-grep run -l kotlin --kind property_declaration "$file" --json 2>/dev/null)"
    if [ -n "$prop_raw" ] && [ "$prop_raw" != "[]" ]; then
        local prop_extracted
        prop_extracted="$(echo "$prop_raw" | jq --arg depth "$depth" '
            [.[] | select(.range.start.column == 0) | {
                kind: (.text | split("\n")[0] | (if test("^(\\s*)var\\b") then "var" else "val" end)),
                name: (.text | split("\n")[0] | capture("(?:val|var)\\s+(?<n>\\w+)") | .n // "?"),
                line: (.range.start.line + 1),
                text: (
                    if $depth == "full" then .text
                    elif $depth == "signatures" then (.text | split("\n")[0])
                    else null
                    end
                )
            }]
        ' 2>/dev/null)"

        if [ -n "$prop_extracted" ] && [ "$prop_extracted" != "[]" ]; then
            all_json="$(echo "$all_json" "$prop_extracted" | jq -s '.[0] + .[1]')"
        fi
    fi

    echo "$all_json" | jq 'sort_by(.line) | if .[0].text == null then [.[] | del(.text)] else . end'
}
