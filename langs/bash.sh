#!/usr/bin/env bash
# Language config: Bash

LANG_ID="bash"
FILE_EXT=".sh"
FILE_EXCLUDE_GLOBS=""
SRC_DIR_REL="."
ENTRY_POINT_REL=""  # set per-project
PACKAGE_NAME=""
CTX7_LIBRARIES="bash"
LSP_PLUGIN=""  # no official Claude Code plugin
LSP_BINARY="bash-language-server"
LSP_INSTALL="npm install -g bash-language-server"

# ─────────────────────────────────────────────────────────
# Skill metadata — used by `discover init` to fill SKILL.md.template
# ─────────────────────────────────────────────────────────
SKILL_LANG_NAME="Bash"
SKILL_IMPORT_VERB="source"
SKILL_MOTIVATION="Bash's \`source\` and \`.\` commands resolve relative to the sourcing file or \$PATH — grep can't distinguish which files actually get loaded at runtime or follow the dependency chain across sourced files. discover.sh resolves the real source graph."
SKILL_RESOLVE_EXAMPLE_1="./lib/utils.sh"
SKILL_RESOLVE_RESULT_1="lib/utils.sh"
SKILL_RESOLVE_EXAMPLE_2="../config.sh"
SKILL_RESOLVE_FROM_2="lib/app/main.sh"
SKILL_RESOLVE_RESULT_2="lib/config.sh"
SKILL_EXAMPLE_FILE="lib/utils.sh"
SKILL_EXAMPLE_SYMBOL="process_file"
SKILL_USAGE_EXAMPLE_1='$FUNC() { $$$BODY }'
SKILL_USAGE_EXAMPLE_2='function $FUNC { $$$BODY }'

# ─────────────────────────────────────────────────────────
# Bash has `source` and `.` (dot) directives
# ─────────────────────────────────────────────────────────
import_directives() { echo "source dot"; }

inspect_sections() { echo "sources dots entities"; }

# ─────────────────────────────────────────────────────────
# Source resolution
#
# Bash source/dot forms:
#   source ./lib/utils.sh        → relative to sourcing file
#   source "/absolute/path.sh"   → absolute path
#   . ./lib/utils.sh             → same as source
#   . "../config.sh"             → relative upward from sourcing file
#
# No package system — all paths are file-relative or absolute.
# ─────────────────────────────────────────────────────────
resolve_import() {
    local specifier="$1" from_file="$2"

    # Strip surrounding quotes if present
    specifier="${specifier#\"}"
    specifier="${specifier%\"}"
    specifier="${specifier#\'}"
    specifier="${specifier%\'}"

    # Best-effort: strip variable/subshell prefixes from paths like
    # $SCRIPT_DIR/lib/utils.sh, ${DIR}/lib/utils.sh, $(dirname $0)/lib/utils.sh
    if [[ "$specifier" == *'$'* ]]; then
        local stripped
        stripped="$(echo "$specifier" | sed -E 's/(\$\{[^}]+\}|\$\([^)]+\)|\$[A-Za-z_][A-Za-z_0-9]*)\/?//')"
        [ -n "$stripped" ] && specifier="$stripped"
    fi

    # Absolute paths — check directly
    if [[ "$specifier" == /* ]]; then
        [ -f "$specifier" ] && echo "$specifier"
        return
    fi

    # Relative to sourcing file
    if [ -n "$from_file" ]; then
        local dir
        dir="$(dirname "$(realpath "$REPO_ROOT/$from_file" 2>/dev/null)")"

        local candidate
        candidate="$(realpath -m "$dir/$specifier" 2>/dev/null)"
        [ -f "$candidate" ] && { realpath --relative-to="$REPO_ROOT" "$candidate" 2>/dev/null; return; }
    fi

    # Relative to repo root
    local root_candidate="$REPO_ROOT/$specifier"
    if [ -f "$root_candidate" ]; then
        realpath --relative-to="$REPO_ROOT" "$root_candidate" 2>/dev/null
        return
    fi

    # Relative to SRC_DIR
    local src_candidate="$REPO_ROOT/$SRC_DIR_REL/$specifier"
    if [ -f "$src_candidate" ]; then
        realpath --relative-to="$REPO_ROOT" "$src_candidate" 2>/dev/null
        return
    fi
}

# ─────────────────────────────────────────────────────────
# ast-grep patterns for Bash's source / . commands
#
# Both `source $URI` and `. $URI` capture quoted and unquoted
# paths. Quoted paths include surrounding quotes in the metavar,
# so we strip them in resolve_import.
# ─────────────────────────────────────────────────────────
ast_grep_imports() {
    local file="$1"
    {
        ast-grep run -l bash -p 'source $URI' "$file" --json 2>/dev/null \
            | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
        ast-grep run -l bash -p '. $URI' "$file" --json 2>/dev/null \
            | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
    } | sed 's/^"//; s/"$//' | sort -u
}

ast_grep_directive_imports() {
    local file="$1" directive="$2"
    case "$directive" in
        source)
            ast-grep run -l bash -p 'source $URI' "$file" --json 2>/dev/null \
                | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null \
                | sed 's/^"//; s/"$//'
            ;;
        dot)
            ast-grep run -l bash -p '. $URI' "$file" --json 2>/dev/null \
                | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null \
                | sed 's/^"//; s/"$//'
            ;;
    esac
}

# ─────────────────────────────────────────────────────────
# Entity extraction
#
# Bash entities:
#   function_definition — all three function declaration forms
# ─────────────────────────────────────────────────────────
ast_grep_entities() {
    local file="$1" depth="${2:-names}"

    local all_json="[]"

    # Functions (covers: name() {}, function name {}, function name() {})
    local fn_raw
    fn_raw="$(ast-grep run -l bash --kind function_definition "$file" --json 2>/dev/null)"
    if [ -n "$fn_raw" ] && [ "$fn_raw" != "[]" ]; then
        local fn_extracted
        fn_extracted="$(echo "$fn_raw" | jq --arg depth "$depth" '
            [.[] | {
                kind: "function",
                name: (.text | split("\n")[0]
                    | if startswith("function ") then
                        ltrimstr("function ") | split(" ")[0] | split("(")[0] | gsub("\\s";"")
                      else
                        split("(")[0] | gsub("\\s";"")
                      end),
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
