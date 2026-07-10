#!/usr/bin/env bash
# Language config template for discover.sh generator.
#
# Copy this file to langs/<language>.sh and fill in every section.
# Functions marked REQUIRED must be implemented. Functions marked OPTIONAL
# have sensible defaults in the core and can be omitted.
#
# The generated discover.sh sources this config at the top, so every
# variable and function defined here is available to the core logic.

# ─────────────────────────────────────────────────────────
# REQUIRED VARIABLES
# ─────────────────────────────────────────────────────────

# ast-grep language identifier (passed to `ast-grep run -l`)
LANG_ID=""

# File extension(s) including the dot (e.g. ".dart", ".cr", ".swift").
# Space-separated for multiple extensions (e.g. ".ts .tsx").
FILE_EXT=""

# Glob patterns to exclude from file discovery (space-separated).
# E.g. "*.g.dart *.freezed.dart" for Dart generated files.
FILE_EXCLUDE_GLOBS=""

# Directory under REPO_ROOT where source files live.
# Relative to REPO_ROOT. E.g. "lib" for Dart, "src" for Crystal, "." for flat layouts.
SRC_DIR_REL=""

# Entry point file, relative to REPO_ROOT.
# E.g. "lib/main.dart" for Flutter, "src/myapp.cr" for Crystal.
# Set to "" if the language has no single entry point (e.g. libraries).
ENTRY_POINT_REL=""

# Package/module name used in self-referencing imports.
# E.g. "my_app" for Dart's `package:my_app/...`.
# Set to "" if the language doesn't use package-qualified imports.
PACKAGE_NAME=""

# Context7 library search terms for this language's ecosystem.
# Space-separated. Used in generated SKILL.md to enable doc lookups.
# E.g. "flutter dart" for Dart, "typescript node.js" for TypeScript.
CTX7_LIBRARIES=""

# Claude Code LSP plugin ID from the official marketplace.
# E.g. "typescript-lsp@claude-plugins-official", "ruby-lsp@claude-plugins-official".
# Leave empty if no official plugin exists (Dart, Crystal, Bash).
LSP_PLUGIN=""

# LSP binary name to check for on PATH.
# E.g. "typescript-language-server", "ruby-lsp", "sourcekit-lsp".
LSP_BINARY=""

# Command to install the LSP binary if not found.
# E.g. "npm install -g typescript-language-server typescript".
# Leave empty if bundled with the language toolchain.
LSP_INSTALL=""

# ─────────────────────────────────────────────────────────
# REQUIRED FUNCTIONS
# ─────────────────────────────────────────────────────────

# resolve_import <import-specifier> [<from-file-repo-relative>]
#
# Given an import/require string as it appears in source, print the
# repo-relative path(s) it resolves to (one per line). For glob imports
# (Crystal's `require "./*"`), print multiple lines. Print nothing for
# unresolvable imports (stdlib, external packages).
#
# $REPO_ROOT and $SRC_DIR are available as globals.
resolve_import() {
    local specifier="$1" from_file="$2"
    # TODO: implement
    :
}

# ast_grep_imports <absolute-file-path>
#
# Extract import/require specifiers from a file using ast-grep.
# Print one specifier per line (the string value, not the full statement).
#
# Example for Dart:
#   ast-grep run -l dart -p "import '\$URI';" "$1" --json \
#       | jq -r '.[].metaVariables.single.URI.text'
#
# Example for Crystal:
#   ast-grep run -l crystal -p 'require "$URI"' "$1" --json \
#       | jq -r '.[].metaVariables.single.URI.text'
ast_grep_imports() {
    local file="$1"
    # TODO: implement
    :
}

# ast_grep_entities <absolute-file-path> <depth>
#
# Extract top-level entities (classes, functions, structs, enums, etc.)
# as a JSON array. Each element:
#   { "kind": "class", "name": "Foo", "line": 17 }
#
# depth is one of: names, signatures, full
#   names      — kind + name + line only (omit "text")
#   signatures — add "text" with the first line
#   full       — add "text" with the entire body
#
# Sort by line number. For "names" depth, omit the "text" field entirely.
ast_grep_entities() {
    local file="$1" depth="${2:-names}"
    # TODO: implement
    echo "[]"
}

# ─────────────────────────────────────────────────────────
# OPTIONAL FUNCTIONS
# Override these only if the language needs special behavior.
# ─────────────────────────────────────────────────────────

# import_directives
#
# Space-separated list of directive types to follow during BFS.
# Default: "import"
# Dart overrides to: "import export part"
# Crystal uses just: "import" (mapped to `require`)
#
# import_directives() { echo "import"; }

# ast_grep_directive_imports <absolute-file-path> <directive>
#
# Like ast_grep_imports but for a specific directive type.
# Only needed if import_directives() returns more than "import".
# Default implementation calls ast_grep_imports for "import" and
# does nothing for other directives.
#
# Dart overrides this to handle import/export/part separately:
#   ast-grep run -l dart -p "${directive} '\$URI';" "$file" --json ...
#
# ast_grep_directive_imports() { local file="$1" directive="$2"; ... }

# inspect_sections
#
# Space-separated list of --flag names for the inspect subcommand.
# Default: "imports entities"
# Dart overrides to: "imports exports parts entities"
#
# inspect_sections() { echo "imports entities"; }

# emit_inspect_section <section-name> <absolute-file-path> <repo-relative-path>
#
# Print JSON for one section of the inspect output.
# Only needed if inspect_sections() includes custom sections.
# Default handles "imports" and "entities".
#
# emit_inspect_section() { local section="$1" abs_file="$2" repo_rel="$3"; ... }

# find_source_files
#
# Print all candidate source files (absolute paths, null-delimited).
# Default: find $SRC_DIR -name "*${FILE_EXT}" excluding FILE_EXCLUDE_GLOBS.
# Handles space-separated FILE_EXT automatically (e.g. ".ts .tsx").
#
# find_source_files() { ... }

# ast_grep_languages
#
# Space-separated list of ast-grep language IDs for directory-level scans
# (e.g. usages). Default: "$LANG_ID".
# Override when the language uses multiple grammars — TypeScript overrides
# to "typescript tsx" because .ts and .tsx files use separate grammars.
#
# ast_grep_languages() { echo "$LANG_ID"; }
