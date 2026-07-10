#!/usr/bin/env bash
# generate.sh — produce a standalone discover.sh from a language config.
#
# Usage:
#   bash generate.sh <lang-config.sh> [--output <path>] [--entry <file>] [--package <name>] [--src-dir <dir>]
#
# Generates a self-contained discover.sh that embeds both the language config
# and the core logic. The output has no external dependencies beyond ast-grep,
# jq, and standard coreutils.
#
# Override per-project settings:
#   --entry <file>    Override ENTRY_POINT_REL (e.g. "src/myapp.cr")
#   --package <name>  Override PACKAGE_NAME (e.g. "my_app")
#   --src-dir <dir>   Override SRC_DIR_REL (e.g. "lib")



set -euo pipefail
#
# SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
#
SELF="$(realpath "${BASH_SOURCE[0]}")"
SELF_DIR="${SELF_DIR:-${SELF%\/*}}"


lang_config=""
output=""
override_entry=""
override_package=""
override_src_dir=""

while [ $# -gt 0 ]; do
    case "$1" in
        --output)  shift; output="$1" ;;
        --entry)   shift; override_entry="$1" ;;
        --package) shift; override_package="$1" ;;
        --src-dir) shift; override_src_dir="$1" ;;
        *) lang_config="$1" ;;
    esac
    shift
done

[ -n "$lang_config" ] || { echo "usage: generate.sh <lang-config.sh> [--output <path>] [--entry <file>] [--package <name>]" >&2; exit 1; }
[ -f "$lang_config" ] || { echo "error: config not found: $lang_config" >&2; exit 1; }

core_file="$SELF_DIR/core.sh"
[ -f "$core_file" ] || { echo "error: core.sh not found at $SELF_DIR" >&2; exit 1; }

# Extract LANG_ID from config for naming
lang_id="$(grep -oP '^LANG_ID="\K[^"]+' "$lang_config")"
[ -n "$lang_id" ] || { echo "error: could not extract LANG_ID from config" >&2; exit 1; }

if [ -z "$output" ]; then
    output="discover-${lang_id}.sh"
fi

{
    echo '#!/usr/bin/env bash'
    echo "# discover-${lang_id}.sh — deterministic ${lang_id} codebase navigation using ast-grep."
    echo "# Generated from: $(basename "$lang_config") + core.sh"
    echo "# Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "#"
    echo "# Do not edit directly — regenerate with:"
    echo "#   bash generate.sh $(basename "$lang_config") --output $(basename "$output")"
    echo ""

    echo "# ═══════════════════════════════════════════════════════════"
    echo "# Language config: ${lang_id}"
    echo "# ═══════════════════════════════════════════════════════════"
    echo ""

    # Emit the lang config, stripping shebang
    sed '1{/^#!/d}' "$lang_config"

    # Apply overrides
    if [ -n "$override_entry" ]; then
        echo ""
        echo "# Override: entry point"
        echo "ENTRY_POINT_REL=\"$override_entry\""
    fi
    if [ -n "$override_package" ]; then
        echo ""
        echo "# Override: package name"
        echo "PACKAGE_NAME=\"$override_package\""
    fi
    if [ -n "$override_src_dir" ]; then
        echo ""
        echo "# Override: source directory"
        echo "SRC_DIR_REL=\"$override_src_dir\""
    fi

    echo ""
    echo "# ═══════════════════════════════════════════════════════════"
    echo "# Core engine (language-agnostic)"
    echo "# ═══════════════════════════════════════════════════════════"
    echo ""

    # Emit core, stripping shebang and the long header comment
    sed '1{/^#!/d}' "$core_file"

    echo ""
    echo "# ═══════════════════════════════════════════════════════════"
    echo "# Entry point"
    echo "# ═══════════════════════════════════════════════════════════"
    echo 'discover_main "$@"'

} > "$output"

chmod +x "$output"
echo "Generated: $output (language: $lang_id)" >&2
