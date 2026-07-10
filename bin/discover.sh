#!/usr/bin/env bash
# discover.sh — multi-language dispatch wrapper.
#
# Routes to language-specific discover-<lang>.sh scripts.
#
# Usage:
#   bash discover.sh --lang typescript inspect src/index.ts
#   bash discover.sh inspect src/index.ts          # auto-detects from file extension
#   bash discover.sh langs                         # list installed languages
#   bash discover.sh --help

SELF="$(realpath "${BASH_SOURCE[0]}")"
SCRIPT_DIR="${SELF%\/*}"

# Extension-to-language mapping (order: longest ext first to avoid prefix collisions)
EXT_LANG_MAP=(
    ".tsx:typescript"
    ".ts:typescript"
    ".dart:dart"
    ".cr:crystal"
    ".kt:kotlin"
    ".rb:ruby"
    ".sh:bash"
    ".swift:swift"
)

die() { echo "error: $*" >&2; exit 1; }

# List installed discover-<lang>.sh scripts
list_langs() {
    local found=false
    for script in "$SCRIPT_DIR"/discover-*.sh; do
        [ -f "$script" ] || continue
        local name
        name="$(basename "$script")"
        name="${name#discover-}"
        name="${name%.sh}"
        echo "$name"
        found=true
    done
    $found || echo "(none)"
}

# Detect language from a file path argument
detect_lang_from_file() {
    local file="$1"
    for entry in "${EXT_LANG_MAP[@]}"; do
        local ext="${entry%%:*}" lang="${entry#*:}"
        case "$file" in
            *"$ext") echo "$lang"; return 0 ;;
        esac
    done
    return 1
}

# Detect language from file arguments in the command line
detect_lang_from_args() {
    for arg in "$@"; do
        # Skip flags
        [[ "$arg" == -* ]] && continue
        local lang
        lang="$(detect_lang_from_file "$arg" 2>/dev/null)" && { echo "$lang"; return 0; }
    done
    return 1
}

# Auto-detect when only one language is installed
detect_single_lang() {
    local langs=()
    for script in "$SCRIPT_DIR"/discover-*.sh; do
        [ -f "$script" ] || continue
        local name
        name="$(basename "$script")"
        name="${name#discover-}"
        name="${name%.sh}"
        langs+=("$name")
    done
    if [ "${#langs[@]}" -eq 1 ]; then
        echo "${langs[0]}"
        return 0
    fi
    return 1
}

show_help() {
    cat <<'HELP'
discover.sh — multi-language structural codebase navigation

Usage:
  bash discover.sh [--lang <lang>] <subcommand> [args...]
  bash discover.sh langs

When --lang is omitted, the language is detected from:
  1. File extension of a file argument (e.g. src/index.ts → typescript)
  2. The only installed language (if just one)

Subcommands are forwarded to the language-specific script.
Run `bash discover.sh --lang <lang> --help` for language-specific help.
HELP
}

# ─────────────────────────────────────────────────────────
# Main dispatch
# ─────────────────────────────────────────────────────────
lang=""

# Handle wrapper-level flags
case "${1:-}" in
    langs)
        list_langs
        exit 0
        ;;
    --help|-h)
        show_help
        exit 0
        ;;
    --lang)
        shift
        lang="${1:-}"
        [ -n "$lang" ] || die "--lang requires a value"
        shift
        ;;
esac

# Auto-detect language if not specified
if [ -z "$lang" ]; then
    lang="$(detect_lang_from_args "$@" 2>/dev/null)" \
        || lang="$(detect_single_lang 2>/dev/null)" \
        || die "could not detect language — use --lang <lang> or pass a file argument. Installed: $(list_langs | tr '\n' ' ')"
fi

# Dispatch to language-specific script
target="$SCRIPT_DIR/discover-${lang}.sh"
[ -f "$target" ] || die "no discover script for '${lang}' — installed: $(list_langs | tr '\n' ' ')"

exec bash "$target" "$@"
