# discover — language-agnostic structural codebase navigation

`discover` is a framework for generating `discover.sh` scripts that use
[ast-grep](https://ast-grep.github.io/) to structurally navigate a codebase.
Every generated script provides the same subcommands; only the
language-specific wiring differs.

## What it replaces

Grep. `discover` answers reachability, definition-lookup, and usage questions
by parsing the actual AST, so commented-out code, dead files, and duplicate
names can't produce false positives.

**The core guarantee:** every answer is deterministic and structural. If
`discover` says a file is dead, it's dead — not "probably unused based on a
text search."

## Install

One command, idempotent (safe to re-run — it exits cleanly when already
current, updates in place when a newer release exists).

Linux, macOS, WSL, Git Bash:

```bash
curl -fsSL https://github.com/asmrtfm/discover/releases/latest/download/install.sh | bash
```

Windows (PowerShell):

```powershell
irm https://github.com/asmrtfm/discover/releases/latest/download/install.ps1 | iex
```

This installs the `discover` CLI, statically-linked `ast-grep`/`sg` binaries
built from the vendored fork (which carries language support upstream lacks),
and a static `jq` if your system has none — to `~/.local` (or
`%LOCALAPPDATA%\discover` on Windows). No sudo, no Rust toolchain, no build.

Every push to `main` publishes a new release automatically; re-running the
installer picks it up. All artifacts are checksum-verified against the
release's `SHA256SUMS`.

## Prerequisites

Handled by the installer. If installing by hand instead:

- [ast-grep](https://ast-grep.github.io/) — the structural search engine
  (use this repo's releases, not upstream — the fork adds languages)
- [jq](https://jqlang.github.io/jq/) — JSON processing
- Standard coreutils (`find`, `realpath`, `md5sum`, `sort`)

## Architecture

```
discover/
├── README.md               ← this file
├── core.sh                 ← shared engine (BFS, caching, CLI dispatch)
├── generate.sh             ← assembles a standalone discover.sh
├── lang-template.sh        ← blank config — copy this for a new language
├── SKILL.md.template       ← Claude Code skill description template
└── langs/
    ├── bash.sh             ← Bash config
    ├── crystal.sh          ← Crystal config
    ├── dart.sh             ← Dart/Flutter config
    ├── kotlin.sh           ← Kotlin config
    ├── ruby.sh             ← Ruby config
    ├── swift.sh            ← Swift config (module-aware)
    └── typescript.sh       ← TypeScript/TSX config (dual grammar)
```

The framework has three layers:

| Layer | File | What it does |
|-------|------|--------------|
| **Language config** | `langs/<lang>.sh` | Defines 6 variables and 3 functions that encode how the language's import system, file layout, and AST node types work |
| **Core engine** | `core.sh` | All shared logic: BFS reachability, fingerprint-based caching, definition lookup via import graph traversal, importers scan, structural usage search, inspect, and CLI dispatch |
| **Generator** | `generate.sh` | Concatenates a lang config + core into a single standalone script with no external dependencies beyond ast-grep and jq |

## Subcommands (every language)

Every generated `discover.sh` exposes the same CLI:

| Command | Purpose |
|---------|---------|
| `resolve <import> [--from <file>]` | Resolve an import/require URI to a repo-relative path |
| `inspect <file> [--<section>...] [--depth names\|signatures\|full]` | Extract structured JSON (imports, entities) from a file |
| `definition <file> <symbol>` | Find where a symbol is defined, following the import graph |
| `live-files [--rebuild-cache]` | BFS from entry point — list all reachable files |
| `is-live <file>` | Check if a file is reachable from the entry point |
| `importers <file>` | Find every file that imports/requires the given file |
| `usages <pattern> [--live-only]` | Structural search for an ast-grep pattern |

### resolve

Translates a language-specific import specifier into a repo-relative file path.
Handles all the forms a language supports: package-qualified, relative, glob,
shard/module, etc. Returns nothing for unresolvable imports (stdlib, external
packages).

```bash
# Dart: package import
bash discover.sh resolve "package:my_app/screens/home.dart"
# → lib/screens/home.dart

# Crystal: relative with glob
bash discover.sh resolve "./*" --from src/myapp.cr
# → src/cli.cr
# → src/config.cr

# Crystal: relative require
bash discover.sh resolve "./config" --from src/myapp/cli.cr
# → src/myapp/config.cr
```

### inspect

Extracts structured JSON from a source file. Sections vary by language
(Dart has `--imports`, `--exports`, `--parts`, `--entities`; Crystal has
`--imports`, `--entities`). When no section flags are given, all are included.

The `--depth` flag controls entity detail:
- **`names`** (default) — kind, name, line number only
- **`signatures`** — adds the first line of each entity
- **`full`** — adds the entire entity body

```bash
bash discover.sh inspect lib/screens/home.dart --entities --depth signatures
```

```json
{
  "file": "lib/screens/home.dart",
  "entities": [
    { "kind": "class", "name": "HomeScreen", "line": 12, "text": "class HomeScreen extends StatefulWidget {" },
    { "kind": "class", "name": "_HomeScreenState", "line": 15, "text": "class _HomeScreenState extends State<HomeScreen> {" }
  ]
}
```

### definition

Finds exactly where a symbol is defined by walking the import graph. Checks
local definitions first, then BFS through imports/requires, following
re-exports and transitive chains.

```bash
bash discover.sh definition lib/screens/home.dart AppNavigation
```

```json
{
  "symbol": "AppNavigation",
  "kind": "class",
  "file": "lib/base/nav.dart",
  "line": 17,
  "via": []
}
```

When the definition is reached through re-exports, `via` lists the
intermediate files:

```json
{
  "symbol": "AsyncButton",
  "kind": "class",
  "file": "lib/widgets/reactive_button.dart",
  "line": 77,
  "via": ["lib/screens/common.dart"]
}
```

Exits 1 with `"error": "not found in import graph"` for symbols defined in
external packages or that don't exist.

### live-files

BFS from the configured entry point, following all import/require/export/part
directives. Outputs every reachable file, one per line.

Results are cached based on a fingerprint of the source file listing.
The cache auto-invalidates when files are added or removed. Use
`--rebuild-cache` to force a refresh after changing import statements.

```bash
bash discover.sh live-files
# lib/main.dart
# lib/app.dart
# lib/screens/home.dart
# ...
# 47 live files
```

### is-live

Quick check for a single file — uses the cached live set.

```bash
bash discover.sh is-live lib/screens/home.dart
# LIVE: lib/screens/home.dart (exit 0)

bash discover.sh is-live lib/old/deprecated_widget.dart
# DEAD: lib/old/deprecated_widget.dart (exit 1)
```

### importers

Scans all source files to find which ones import/require/export the given
file. Detects both package-qualified and relative URI forms.

```bash
bash discover.sh importers lib/base/nav.dart
```

```json
[
  { "file": "lib/screens/home.dart", "directive": "import", "specifier": "package:my_app/base/nav.dart" },
  { "file": "lib/screens/settings.dart", "directive": "import", "specifier": "../base/nav.dart" }
]
```

### usages

Structural search using any valid ast-grep pattern for the language.
Metavariables (`$NAME`, `$$$ARGS`) match structurally — this is not regex.

```bash
# Find all usages of a class method pattern
bash discover.sh usages 'AppIcons.$FIELD' --live-only

# Find constructor calls with any arguments
bash discover.sh usages 'OverlayButton.$CTOR($$$ARGS)'
```

`--live-only` filters results to files reachable from the entry point, so
you won't get hits in dead code.

## Generating a standalone discover.sh

```bash
bash lib/generate.sh <lang-config> [--output <path>] [--entry <file>] [--package <name>] [--src-dir <dir>]
```

The generator concatenates the lang config and core engine into a single
self-contained script. Per-project settings can be overridden at generation
time:

```bash
# Dart project
bash lib/generate.sh langs/dart.sh \
  --output scripts/discover.sh \
  --package my_app \
  --entry lib/main.dart

# Crystal project
bash lib/generate.sh langs/crystal.sh \
  --output tools/discover.sh \
  --entry src/myapp.cr

# Swift SPM project
bash lib/generate.sh langs/swift.sh \
  --output scripts/discover.sh \
  --src-dir Sources
```

## Adding a new language

### Step 1: Copy the template

```bash
cp lib/lang-template.sh langs/<lang>.sh
```

### Step 2: Fill in the config

Every lang config defines **6 variables** and **3 functions**. Open
`lang-template.sh` for full documentation of each; here's the summary:

#### Required variables

| Variable | Purpose | Examples |
|----------|---------|----------|
| `LANG_ID` | ast-grep `-l` identifier | `"dart"`, `"crystal"`, `"swift"`, `"python"` |
| `FILE_EXT` | Source file extension(s) with dot, space-separated for multiple | `".dart"`, `".cr"`, `".ts .tsx"` |
| `FILE_EXCLUDE_GLOBS` | Space-separated globs to skip | `"*.g.dart *.freezed.dart"`, `""` |
| `SRC_DIR_REL` | Source directory relative to repo root | `"lib"`, `"src"`, `"Sources"`, `"."` |
| `ENTRY_POINT_REL` | Entry file for reachability analysis | `"lib/main.dart"`, `"src/myapp.cr"`, `""` |
| `PACKAGE_NAME` | Self-referencing import prefix | `"my_app"`, `""` |

#### Required functions

**`resolve_import <specifier> [<from-file>]`** — the most complex piece.
Given an import string exactly as it appears in source code, resolve it to
one or more repo-relative file paths. Return nothing for unresolvable imports
(stdlib, external packages). For glob imports (Crystal's `require "./*"`),
print multiple lines.

This is where each language's import semantics live:

| Language | Forms to handle |
|----------|----------------|
| Dart | `package:pkg/path`, `dart:lib` (skip), relative paths |
| Crystal | `./relative`, `../up`, `./*` glob, `./**` recursive, `"shard"`, `"shard/sub"`, stdlib (skip) |
| Swift | `import Module` → check `Sources/Module/`, external (skip) |
| Python | `from pkg.mod import X` → resolve to `pkg/mod.py` or `pkg/mod/__init__.py` |
| Ruby | `require_relative "./foo"`, `require "gem"` (skip) |
| Go | `"github.com/org/repo/pkg"` → check local vendor or module path |

**`ast_grep_imports <absolute-file-path>`** — extract import specifiers
from a file using ast-grep. Print one specifier per line (the string value,
not the full statement).

Each language has a different pattern:

```bash
# Dart:  import 'package:my_app/foo.dart';
ast-grep run -l dart -p "import '\$URI';" "$file" --json | jq -r '.[].metaVariables.single.URI.text'

# Crystal:  require "./foo"
ast-grep run -l crystal -p 'require "$URI"' "$file" --json | jq -r '.[].metaVariables.single.URI.text'

# Swift:  import Foundation
ast-grep run -l swift -p 'import $MODULE' "$file" --json | jq -r '.[].metaVariables.single.MODULE.text'

# Python:  import foo  /  from foo import bar
ast-grep run -l python -p 'import $MODULE' "$file" --json | jq -r '.[].metaVariables.single.MODULE.text'

# Ruby:  require_relative "./foo"
ast-grep run -l ruby -p 'require_relative "$PATH"' "$file" --json | jq -r '.[].metaVariables.single.PATH.text'
```

**`ast_grep_entities <absolute-file-path> <depth>`** — extract top-level
definitions as a JSON array. Each element has `kind`, `name`, `line`, and
optionally `text` (for `signatures` or `full` depth). The entity types vary
by language:

| Language | Entity kinds to extract | AST node kinds |
|----------|----------------------|----------------|
| Dart | class, enum, mixin, extension, typedef, function | `class_declaration`, `enum_declaration`, `mixin_declaration`, `extension_declaration`, `type_alias`, `function_signature` |
| Crystal | class, module, enum, struct, def, macro | `class_def`, `module_def`, `enum_def`, `struct_def`, `method_def`, `macro_def` |
| Swift | class, struct, enum, protocol, actor, func, extension | `class_declaration`, `struct_declaration`, `enum_declaration`, `protocol_declaration`, `actor_declaration`, `function_declaration`, `extension_declaration` |
| Python | class, function | `class_definition`, `function_definition` |
| Ruby | class, module, def | `class`, `module`, `method` |

#### Optional functions

Override these only when the language needs non-default behavior:

| Function | Default | When to override |
|----------|---------|-----------------|
| `import_directives` | `"import"` | Language has multiple linking directives (Dart: `"import export part"`) |
| `ast_grep_directive_imports` | Delegates to `ast_grep_imports` for `"import"` | Language needs different ast-grep patterns per directive type |
| `inspect_sections` | `"imports entities"` | Language has extra sections (Dart: `"imports exports parts entities"`) |
| `emit_inspect_section` | Builds JSON from directive imports | Custom inspect output format needed |
| `find_source_files` | `find $SRC_DIR -name "*$FILE_EXT" ...` (handles multi-ext) | Non-standard file layout |
| `ast_grep_languages` | `echo "$LANG_ID"` | Language uses multiple ast-grep grammars for directory scans (TypeScript: `"typescript tsx"`) |

### Step 3: Generate and test

```bash
# Generate
bash lib/generate.sh langs/<lang>.sh \
  --output /path/to/project/scripts/discover.sh \
  --entry <entry-point>

# Test individual subcommands
cd /path/to/project
bash scripts/discover.sh inspect <some-file> --entities
bash scripts/discover.sh live-files
bash scripts/discover.sh usages '<some-pattern>'
```

### Step 4: Add the Claude Code skill

Copy `SKILL.md.template` to your project's `.claude/skills/discover/SKILL.md`
and fill in the `{{placeholders}}` with language-specific examples. This tells
Claude Code when and how to use discover instead of grep.

## Caching

`live-files` and `is-live` use a fingerprint-based cache stored in
`<repo>/tmp/`. The fingerprint is an MD5 of the sorted source file listing,
so it auto-invalidates when files are added or removed.

**The cache does not detect import-content changes** (e.g., adding a new
import line to an existing file). Use `--rebuild-cache` after restructuring
imports:

```bash
bash discover.sh live-files --rebuild-cache
```

Cache files:
- `tmp/discover_live_cache.txt` — the cached file list
- `tmp/discover_live_cache.fingerprint` — the fingerprint hash

Add both to `.gitignore`.

## Language notes

### Dart / Flutter

Dart has three directive types that link files: `import`, `export`, and `part`.
The Dart config overrides `import_directives` and `inspect_sections` to handle
all three. The `definition` subcommand follows re-export chains to find the
original source.

The `PACKAGE_NAME` variable must match your `pubspec.yaml` name field for
`package:` import resolution to work.

### Crystal

Crystal's `require` supports glob patterns (`"./*"` and `"./**"`) that resolve
to multiple files. The resolver handles these by returning multiple lines.
Crystal also has a directory-as-module convention where `require "./foo"` can
resolve to `foo/foo.cr` if `foo/` is a directory.

Shard requires (`require "some_shard"`) resolve to `lib/<shard>/src/<shard>.cr`.

### TypeScript / TSX

TypeScript and TSX use separate tree-sitter grammars in ast-grep (`-l typescript` vs `-l tsx`). The TypeScript lang config handles this transparently:

- `FILE_EXT=".ts .tsx"` — `find_source_files` discovers both extensions
- `_ts_lang()` — dispatches `-l tsx` for `.tsx` files, `-l typescript` for `.ts` files
- `ast_grep_languages()` — returns `"typescript tsx"` so directory-level scans (e.g. `usages`) cover both grammars

This means `discover.sh` works unchanged for pure TypeScript projects, React (`.tsx`), and React Native projects. Import resolution already tries `.tsx` extensions; the fix ensures file discovery and entity extraction also work on `.tsx` files.

### Swift

Swift's import system is module-based, not file-based. Within a single
target/module, all `.swift` files see each other without explicit imports.
This means `live-files` is less useful for single-target projects (all files
are live by definition).

The Swift config is most useful for `inspect`, `definition`, `usages`, and
multi-target SPM projects where `import MyOtherModule` can be resolved to a
local `Sources/MyOtherModule/` directory.

## Extending discover for a project

Some projects need extra subcommands beyond the standard set. Two approaches:

1. **Add to the lang config** — define additional `cmd_*` functions in your
   lang config and add cases to a custom dispatch. The generator will include
   them in the output.

2. **Wrapper script** — write a thin wrapper that handles project-specific
   commands and delegates the rest to the generated `discover.sh`. This keeps
   the generated script clean and re-generable.

The Dart config in `malsi-app` uses approach (2) for its `--assets` flag on
the `usages` subcommand, which does a two-phase search (constant field lookup
+ direct string literal search) specific to Flutter asset management.
