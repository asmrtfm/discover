#!/usr/bin/env bash
# Language config: Dart / Flutter

LANG_ID="dart"
FILE_EXT=".dart"
FILE_EXCLUDE_GLOBS="*.g.dart *.freezed.dart"
SRC_DIR_REL="lib"
ENTRY_POINT_REL="lib/main.dart"
PACKAGE_NAME=""  # set per-project, e.g. "my_app"
CTX7_LIBRARIES="flutter dart"
LSP_PLUGIN=""  # no official Claude Code LSP plugin
LSP_BINARY="dart"
LSP_INSTALL=""  # bundled with Dart SDK

# ─────────────────────────────────────────────────────────
# Skill metadata — used by `discover init` to fill SKILL.md.template
# ─────────────────────────────────────────────────────────
SKILL_LANG_NAME="Dart"
SKILL_IMPORT_VERB="import"
SKILL_MOTIVATION="Dart has three linking directives (\`import\`, \`export\`, \`part\`) and two URI schemes (\`package:\`, relative). A grep hit doesn't tell you whether the file is reachable or which definition wins when names collide across barrel exports. discover.sh resolves the real import graph."
SKILL_RESOLVE_EXAMPLE_1="package:my_app/screens/home.dart"
SKILL_RESOLVE_RESULT_1="lib/screens/home.dart"
SKILL_RESOLVE_EXAMPLE_2="../widgets/button.dart"
SKILL_RESOLVE_FROM_2="lib/screens/home.dart"
SKILL_RESOLVE_RESULT_2="lib/widgets/button.dart"
SKILL_EXAMPLE_FILE="lib/screens/home.dart"
SKILL_EXAMPLE_SYMBOL="HomeScreen"
SKILL_USAGE_EXAMPLE_1='Navigator.push($$$ARGS)'
SKILL_USAGE_EXAMPLE_2='setState(() { $$$BODY })'

# ─────────────────────────────────────────────────────────
# Dart has three directive types that link files
# ─────────────────────────────────────────────────────────
import_directives() { echo "import export part"; }

inspect_sections() { echo "imports exports parts entities"; }

# ─────────────────────────────────────────────────────────
# Import resolution
#
# Dart import forms:
#   package:<pkg>/<path>  — self-package resolves to lib/<path>
#   dart:<lib>            — stdlib, unresolvable
#   package:<other>/<..>  — external, unresolvable
#   <relative-path>       — relative to the importing file
# ─────────────────────────────────────────────────────────
resolve_import() {
    local uri="$1" from_file="$2"
    local lib_dir="$REPO_ROOT/$SRC_DIR_REL"

    case "$uri" in
        dart:*)
            return ;;
        package:${PACKAGE_NAME}/*)
            local rel="${uri#package:${PACKAGE_NAME}/}"
            local full="$lib_dir/$rel"
            [ -f "$full" ] && realpath --relative-to="$REPO_ROOT" "$full" 2>/dev/null
            ;;
        package:*)
            return ;;
        *)
            if [ -n "$from_file" ]; then
                local abs_from="$REPO_ROOT/$from_file"
                [ -f "$abs_from" ] || abs_from="$from_file"
                local dir
                dir="$(dirname "$(realpath "$abs_from" 2>/dev/null || echo "$abs_from")")"
                local resolved
                resolved="$(realpath -m "$dir/$uri" 2>/dev/null)"
                [ -f "$resolved" ] && realpath --relative-to="$REPO_ROOT" "$resolved" 2>/dev/null
            fi
            ;;
    esac
}

# ─────────────────────────────────────────────────────────
# ast-grep patterns for Dart's import/export/part directives
# ─────────────────────────────────────────────────────────
ast_grep_imports() {
    ast_grep_directive_imports "$1" "import"
}

ast_grep_directive_imports() {
    local file="$1" directive="$2"
    ast-grep run -l dart -p "${directive} '\$URI';" "$file" --json 2>/dev/null \
        | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
}

# ─────────────────────────────────────────────────────────
# Entity extraction
# ─────────────────────────────────────────────────────────
ast_grep_entities() {
    local file="$1" depth="${2:-names}"
    local kinds="class_declaration enum_declaration mixin_declaration extension_declaration type_alias"
    local keyword_map="class_declaration:class enum_declaration:enum mixin_declaration:mixin extension_declaration:extension type_alias:typedef"

    local all_json="[]"

    for kind in $kinds; do
        local keyword=""
        for pair in $keyword_map; do
            [ "${pair%%:*}" = "$kind" ] && { keyword="${pair#*:}"; break; }
        done

        local raw
        raw="$(ast-grep run -l dart --kind "$kind" "$file" --json 2>/dev/null)"
        [ -n "$raw" ] && [ "$raw" != "[]" ] || continue

        local extracted
        extracted="$(echo "$raw" | jq --arg kw "$keyword" --arg depth "$depth" '
            [.[] | {
                kind: $kw,
                name: (.text | split("\n")[0] | capture("(?:^|\\s)" + $kw + "\\s+(?<n>\\w+)") | .n // "?"),
                line: (.range.start.line + 1),
                text: (
                    if $depth == "full" then .text
                    elif $depth == "signatures" then (.text | split("\n")[0])
                    else null
                    end
                )
            }]
        ' 2>/dev/null)"

        [ -n "$extracted" ] && [ "$extracted" != "[]" ] || continue
        all_json="$(echo "$all_json" "$extracted" | jq -s '.[0] + .[1]')"
    done

    # Top-level functions (column 0)
    local fn_raw
    fn_raw="$(ast-grep run -l dart --kind function_signature "$file" --json 2>/dev/null)"
    if [ -n "$fn_raw" ] && [ "$fn_raw" != "[]" ]; then
        local fn_extracted
        fn_extracted="$(echo "$fn_raw" | jq --arg depth "$depth" '
            [.[] | select(.range.start.column == 0) | {
                kind: "function",
                name: (.text | capture("(?<n>\\w+)\\s*\\(") | .n // "?"),
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

    echo "$all_json" | jq 'sort_by(.line) | if .[0].text == null then [.[] | del(.text)] else . end'
}
