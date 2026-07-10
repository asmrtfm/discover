#!/usr/bin/env bash
# Language config: Crystal

LANG_ID="crystal"
FILE_EXT=".cr"
FILE_EXCLUDE_GLOBS=""
SRC_DIR_REL="src"
ENTRY_POINT_REL=""  # set per-project, e.g. "src/myapp.cr"
PACKAGE_NAME=""
CTX7_LIBRARIES="crystal-lang"
LSP_PLUGIN=""  # no official Claude Code plugin
LSP_BINARY="crystalline"
LSP_INSTALL=""  # build from source or shards

# ─────────────────────────────────────────────────────────
# Skill metadata — used by `discover init` to fill SKILL.md.template
# ─────────────────────────────────────────────────────────
SKILL_LANG_NAME="Crystal"
SKILL_IMPORT_VERB="require"
SKILL_MOTIVATION="Crystal's \`require\` resolves relative paths, globs, shards, and stdlib — grep can't distinguish these. A file found by grep may be in a shard you don't use, or behind a glob you've already covered. discover.sh resolves the actual require graph."
SKILL_RESOLVE_EXAMPLE_1="./config"
SKILL_RESOLVE_RESULT_1="src/config.cr"
SKILL_RESOLVE_EXAMPLE_2="./models/*"
SKILL_RESOLVE_FROM_2="src/app.cr"
SKILL_RESOLVE_RESULT_2="src/models/user.cr src/models/post.cr"
SKILL_EXAMPLE_FILE="src/app.cr"
SKILL_EXAMPLE_SYMBOL="App"
SKILL_USAGE_EXAMPLE_1='App.new($$$ARGS)'
SKILL_USAGE_EXAMPLE_2='Log.info { $$$MSG }'

# ─────────────────────────────────────────────────────────
# Crystal has a single `require` directive
# ─────────────────────────────────────────────────────────
import_directives() { echo "import"; }

inspect_sections() { echo "imports entities"; }

# ─────────────────────────────────────────────────────────
# Require resolution
#
# Crystal require forms:
#   "./foo"      → relative to the requiring file, appends .cr
#   "./foo/*"    → glob: all .cr files in that directory
#   "./foo/**"   → glob: recursive
#   "../foo"     → relative upward
#   "foo"        → stdlib or shard (lib/foo/src/foo.cr)
#   "foo/bar"    → shard sub-path (lib/foo/src/foo/bar.cr)
# ─────────────────────────────────────────────────────────
resolve_import() {
    local req="$1" from_file="$2"
    local lib_dir="$REPO_ROOT/lib"

    # Glob: recursive
    if [[ "$req" == *"/**" ]]; then
        local base="${req%/\*\*}"
        if [[ "$base" == ./* ]] && [ -n "$from_file" ]; then
            local dir
            dir="$(dirname "$(realpath "$REPO_ROOT/$from_file" 2>/dev/null)")"
            local resolved_dir
            resolved_dir="$(realpath -m "$dir/${base#./}" 2>/dev/null)"
            if [ -d "$resolved_dir" ]; then
                find "$resolved_dir" -name "*.cr" -print0 2>/dev/null | sort -z | while IFS= read -r -d '' f; do
                    realpath --relative-to="$REPO_ROOT" "$f" 2>/dev/null
                done
            fi
        fi
        return
    fi

    # Glob: single level
    if [[ "$req" == *"/*" ]]; then
        local base="${req%/\*}"
        if [[ "$base" == ./* ]] && [ -n "$from_file" ]; then
            local dir
            dir="$(dirname "$(realpath "$REPO_ROOT/$from_file" 2>/dev/null)")"
            local resolved_dir
            resolved_dir="$(realpath -m "$dir/${base#./}" 2>/dev/null)"
            if [ -d "$resolved_dir" ]; then
                find "$resolved_dir" -maxdepth 1 -name "*.cr" -print0 2>/dev/null | sort -z | while IFS= read -r -d '' f; do
                    realpath --relative-to="$REPO_ROOT" "$f" 2>/dev/null
                done
            fi
        fi
        return
    fi

    # Relative require
    if [[ "$req" == ./* ]] || [[ "$req" == ../* ]]; then
        if [ -n "$from_file" ]; then
            local dir
            dir="$(dirname "$(realpath "$REPO_ROOT/$from_file" 2>/dev/null)")"

            # Try .cr extension
            local candidate
            candidate="$(realpath -m "$dir/$req.cr" 2>/dev/null)"
            [ -f "$candidate" ] && { realpath --relative-to="$REPO_ROOT" "$candidate" 2>/dev/null; return; }

            # Try directory with same-named file inside
            local dir_candidate
            dir_candidate="$(realpath -m "$dir/$req" 2>/dev/null)"
            local basename
            basename="$(basename "$req")"
            if [ -d "$dir_candidate" ] && [ -f "$dir_candidate/$basename.cr" ]; then
                realpath --relative-to="$REPO_ROOT" "$dir_candidate/$basename.cr" 2>/dev/null
                return
            fi

            # Try bare filename
            candidate="$(realpath -m "$dir/$req" 2>/dev/null)"
            [ -f "$candidate" ] && { realpath --relative-to="$REPO_ROOT" "$candidate" 2>/dev/null; return; }
        fi
        return
    fi

    # Shard: "foo" → lib/foo/src/foo.cr
    local shard_path="$lib_dir/$req/src/$req.cr"
    if [ -f "$shard_path" ]; then
        realpath --relative-to="$REPO_ROOT" "$shard_path" 2>/dev/null
        return
    fi

    # Shard sub-path: "foo/bar" → lib/foo/src/foo/bar.cr
    local shard_name="${req%%/*}"
    local sub_path="${req#*/}"
    local shard_sub="$lib_dir/$shard_name/src/$sub_path.cr"
    if [ -f "$shard_sub" ]; then
        realpath --relative-to="$REPO_ROOT" "$shard_sub" 2>/dev/null
        return
    fi

    # Stdlib — no resolution
}

# ─────────────────────────────────────────────────────────
# ast-grep pattern for Crystal's require
# ─────────────────────────────────────────────────────────
ast_grep_imports() {
    local file="$1"
    ast-grep run -l crystal -p 'require "$URI"' "$file" --json 2>/dev/null \
        | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
}

# ─────────────────────────────────────────────────────────
# Entity extraction
# ─────────────────────────────────────────────────────────
ast_grep_entities() {
    local file="$1" depth="${2:-names}"
    local kinds="class_def module_def enum_def struct_def"
    local keyword_map="class_def:class module_def:module enum_def:enum struct_def:struct"

    local all_json="[]"

    for kind in $kinds; do
        local keyword=""
        for pair in $keyword_map; do
            [ "${pair%%:*}" = "$kind" ] && { keyword="${pair#*:}"; break; }
        done

        local raw
        raw="$(ast-grep run -l crystal --kind "$kind" "$file" --json 2>/dev/null)"
        [ -n "$raw" ] && [ "$raw" != "[]" ] || continue

        local extracted
        extracted="$(echo "$raw" | jq --arg kw "$keyword" --arg depth "$depth" '
            [.[] | {
                kind: $kw,
                name: (.text | split("\n")[0] | capture("(?:^|\\s)" + $kw + "\\s+(?<n>[A-Z]\\w*)") | .n // "?"),
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

    # Methods
    local fn_raw
    fn_raw="$(ast-grep run -l crystal --kind method_def "$file" --json 2>/dev/null)"
    if [ -n "$fn_raw" ] && [ "$fn_raw" != "[]" ]; then
        local fn_extracted
        fn_extracted="$(echo "$fn_raw" | jq --arg depth "$depth" '
            [.[] | {
                kind: "def",
                name: (.text | split("\n")[0] | capture("def\\s+(?:self\\.)?(?<n>\\w+)") | .n // "?"),
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

    # Macros
    local macro_raw
    macro_raw="$(ast-grep run -l crystal --kind macro_def "$file" --json 2>/dev/null)"
    if [ -n "$macro_raw" ] && [ "$macro_raw" != "[]" ]; then
        local macro_extracted
        macro_extracted="$(echo "$macro_raw" | jq --arg depth "$depth" '
            [.[] | {
                kind: "macro",
                name: (.text | split("\n")[0] | capture("macro\\s+(?<n>\\w+)") | .n // "?"),
                line: (.range.start.line + 1),
                text: (
                    if $depth == "full" then .text
                    elif $depth == "signatures" then (.text | split("\n")[0])
                    else null
                    end
                )
            }]
        ' 2>/dev/null)"

        if [ -n "$macro_extracted" ] && [ "$macro_extracted" != "[]" ]; then
            all_json="$(echo "$all_json" "$macro_extracted" | jq -s '.[0] + .[1]')"
        fi
    fi

    echo "$all_json" | jq 'sort_by(.line) | if .[0].text == null then [.[] | del(.text)] else . end'
}
