#!/usr/bin/env bash
# Language config: Ruby

LANG_ID="ruby"
FILE_EXT=".rb"
FILE_EXCLUDE_GLOBS=""
SRC_DIR_REL="lib"
ENTRY_POINT_REL=""  # set per-project, e.g. "lib/myapp.rb"
PACKAGE_NAME=""
CTX7_LIBRARIES="ruby"
LSP_PLUGIN="ruby-lsp@claude-plugins-official"
LSP_BINARY="ruby-lsp"
LSP_INSTALL="gem install ruby-lsp"

# ─────────────────────────────────────────────────────────
# Skill metadata — used by `discover init` to fill SKILL.md.template
# ─────────────────────────────────────────────────────────
SKILL_LANG_NAME="Ruby"
SKILL_IMPORT_VERB="require"
SKILL_MOTIVATION="Ruby's \`require\` and \`require_relative\` resolve through \$LOAD_PATH and file-relative paths respectively — grep can't distinguish a gem's file from your own, or tell you if a require_relative chain actually reaches a file. discover.sh resolves the real require graph."
SKILL_RESOLVE_EXAMPLE_1="./models/user"
SKILL_RESOLVE_RESULT_1="lib/models/user.rb"
SKILL_RESOLVE_EXAMPLE_2="../config"
SKILL_RESOLVE_FROM_2="lib/app/server.rb"
SKILL_RESOLVE_RESULT_2="lib/config.rb"
SKILL_EXAMPLE_FILE="lib/app.rb"
SKILL_EXAMPLE_SYMBOL="App"
SKILL_USAGE_EXAMPLE_1='MyClass.new($$$ARGS)'
SKILL_USAGE_EXAMPLE_2='def $METHOD($$$PARAMS)'

# ─────────────────────────────────────────────────────────
# Ruby has require and require_relative directives
# ─────────────────────────────────────────────────────────
import_directives() { echo "import require_relative"; }

inspect_sections() { echo "imports require_relatives entities"; }

# ─────────────────────────────────────────────────────────
# Require resolution
#
# Ruby require forms:
#   require "foo"            → gem or $LOAD_PATH lookup, unresolvable
#   require "foo/bar"        → gem sub-path, unresolvable
#   require_relative "./foo" → relative to requiring file, appends .rb
#   require_relative "foo"   → relative to requiring file, appends .rb
#   require_relative "../x"  → relative upward, appends .rb
# ─────────────────────────────────────────────────────────
resolve_import() {
    local specifier="$1" from_file="$2"

    # require_relative is always relative to the requiring file
    # require with a relative-looking path is also file-relative
    if [ -n "$from_file" ]; then
        local dir
        dir="$(dirname "$(realpath "$REPO_ROOT/$from_file" 2>/dev/null)")"

        # Try with .rb extension
        local candidate
        candidate="$(realpath -m "$dir/$specifier.rb" 2>/dev/null)"
        [ -f "$candidate" ] && { realpath --relative-to="$REPO_ROOT" "$candidate" 2>/dev/null; return; }

        # Try bare path (already has extension or is extensionless)
        candidate="$(realpath -m "$dir/$specifier" 2>/dev/null)"
        [ -f "$candidate" ] && { realpath --relative-to="$REPO_ROOT" "$candidate" 2>/dev/null; return; }

        # Try as directory with same-named file inside
        if [ -d "$candidate" ]; then
            local basename
            basename="$(basename "$specifier")"
            if [ -f "$candidate/$basename.rb" ]; then
                realpath --relative-to="$REPO_ROOT" "$candidate/$basename.rb" 2>/dev/null
                return
            fi
        fi
    fi

    # For bare require "foo" — check lib/foo.rb and lib/foo/foo.rb
    local lib_candidate="$REPO_ROOT/$SRC_DIR_REL/$specifier.rb"
    if [ -f "$lib_candidate" ]; then
        realpath --relative-to="$REPO_ROOT" "$lib_candidate" 2>/dev/null
        return
    fi

    local lib_dir_candidate="$REPO_ROOT/$SRC_DIR_REL/$specifier"
    local base_name="${specifier##*/}"
    if [ -d "$lib_dir_candidate" ] && [ -f "$lib_dir_candidate/$base_name.rb" ]; then
        realpath --relative-to="$REPO_ROOT" "$lib_dir_candidate/$base_name.rb" 2>/dev/null
        return
    fi

    # External gem or stdlib — no resolution
}

# ─────────────────────────────────────────────────────────
# ast-grep patterns for Ruby's require / require_relative
# ─────────────────────────────────────────────────────────
ast_grep_imports() {
    local file="$1"
    ast-grep run -l ruby -p 'require "$URI"' "$file" --json 2>/dev/null \
        | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
}

ast_grep_directive_imports() {
    local file="$1" directive="$2"
    case "$directive" in
        import)
            ast-grep run -l ruby -p 'require "$URI"' "$file" --json 2>/dev/null \
                | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
            ;;
        require_relative)
            ast-grep run -l ruby -p 'require_relative "$URI"' "$file" --json 2>/dev/null \
                | jq -r '.[].metaVariables.single.URI.text' 2>/dev/null
            ;;
    esac
}

# ─────────────────────────────────────────────────────────
# Entity extraction
# ─────────────────────────────────────────────────────────
ast_grep_entities() {
    local file="$1" depth="${2:-names}"
    local kinds="module class"
    local keyword_map="module:module class:class"

    local all_json="[]"

    for kind in $kinds; do
        local keyword=""
        for pair in $keyword_map; do
            [ "${pair%%:*}" = "$kind" ] && { keyword="${pair#*:}"; break; }
        done

        local raw
        raw="$(ast-grep run -l ruby --kind "$kind" "$file" --json 2>/dev/null)"
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

    # Instance methods
    local fn_raw
    fn_raw="$(ast-grep run -l ruby --kind method "$file" --json 2>/dev/null)"
    if [ -n "$fn_raw" ] && [ "$fn_raw" != "[]" ]; then
        local fn_extracted
        fn_extracted="$(echo "$fn_raw" | jq --arg depth "$depth" '
            [.[] | {
                kind: "def",
                name: (.text | split("\n")[0] | capture("def\\s+(?<n>\\w+[?!=]?)") | .n // "?"),
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

    # Singleton methods (def self.foo)
    local sm_raw
    sm_raw="$(ast-grep run -l ruby --kind singleton_method "$file" --json 2>/dev/null)"
    if [ -n "$sm_raw" ] && [ "$sm_raw" != "[]" ]; then
        local sm_extracted
        sm_extracted="$(echo "$sm_raw" | jq --arg depth "$depth" '
            [.[] | {
                kind: "def self",
                name: (.text | split("\n")[0] | capture("def\\s+self\\.(?<n>\\w+[?!=]?)") | .n // "?"),
                line: (.range.start.line + 1),
                text: (
                    if $depth == "full" then .text
                    elif $depth == "signatures" then (.text | split("\n")[0])
                    else null
                    end
                )
            }]
        ' 2>/dev/null)"

        if [ -n "$sm_extracted" ] && [ "$sm_extracted" != "[]" ]; then
            all_json="$(echo "$all_json" "$sm_extracted" | jq -s '.[0] + .[1]')"
        fi
    fi

    echo "$all_json" | jq 'sort_by(.line) | if .[0].text == null then [.[] | del(.text)] else . end'
}
