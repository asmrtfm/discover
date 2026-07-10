---
name: discover
description: Structural Dart codebase navigation using ast-grep. Resolves imports, inspects files, finds definitions, traces reachability, and locates usages via AST. Prefer over grep for reachability questions (is this file live?) and definition lookups (where is this symbol defined?).
---

## Why this exists

Dart has three linking directives (`import`, `export`, `part`) and two URI schemes (`package:`, relative). A grep hit doesn't tell you whether the file is reachable or which definition wins when names collide across barrel exports. discover.sh resolves the real import graph.

**Use `discover.sh` before acting on any grep hit.** If you grep for a symbol and find it, you still don't know if the file is live or if that's even the right definition. `discover.sh` answers those questions.

## Script location

```
bin/discover/discover-dart.sh
```

Multi-language wrapper (auto-detects language from file extensions):
```
bin/discover/discover.sh
```

Run from the repo root. All file paths are relative to repo root.

## Subcommands

### resolve

Resolve a Dart import to a repo-relative file path.

```bash
bash bin/discover/discover-dart.sh resolve "package:my_app/screens/home.dart"
# → lib/screens/home.dart

bash bin/discover/discover-dart.sh resolve "../widgets/button.dart" --from lib/screens/home.dart
# → lib/widgets/button.dart
```

### inspect

Extract structured JSON from a Dart file. Filter with --imports --exports --parts --entities. Default is all.

`--depth` controls entity detail: `names` (default), `signatures` (first line), `full` (entire body).

```bash
bash bin/discover/discover-dart.sh inspect lib/screens/home.dart --imports
bash bin/discover/discover-dart.sh inspect lib/screens/home.dart --entities --depth signatures
bash bin/discover/discover-dart.sh inspect lib/screens/home.dart --entities --depth full
```

### definition

Find the exact file and line where a symbol used in a file is defined. Follows the file's import chain. Never guesses.

```bash
bash bin/discover/discover-dart.sh definition lib/screens/home.dart HomeScreen
# → {"symbol":"HomeScreen","kind":"class","file":"...","line":17,"via":[]}
```

Exits 1 with `"error": "not found in import graph"` for external packages or symbols that don't exist.

### live-files

BFS from the entry point following all imports. Outputs every reachable file, one per line.

```bash
bash bin/discover/discover-dart.sh live-files
```

### is-live

Check whether a single file is reachable from the entry point.

```bash
bash bin/discover/discover-dart.sh is-live lib/screens/home.dart
# → LIVE: lib/screens/home.dart (exit 0)
```

### importers

Find every file that imports the given file.

```bash
bash bin/discover/discover-dart.sh importers lib/screens/home.dart
```

### usages

Find all references to an ast-grep pattern. `--live-only` filters to files reachable from the entry point.

```bash
bash bin/discover/discover-dart.sh usages 'Navigator.push($$$ARGS)' --live-only
bash bin/discover/discover-dart.sh usages 'setState(() { $$$BODY })'
```

The pattern is any valid ast-grep Dart pattern — metavariables (`$NAME`, `$$$ARGS`) match structurally.

## Tests

```bash
bash bin/discover/test-discover-dart.sh
```

## Documentation lookup

When you need current API docs, import semantics, or framework-specific behavior for this project's language, use Context7:

```bash
npx ctx7@latest library "flutter" "<your question>"
npx ctx7@latest docs "<library-id>" "<your question>"
npx ctx7@latest library "dart" "<your question>"
npx ctx7@latest docs "<library-id>" "<your question>"
```

Use this before relying on training data for Dart API details, configuration options, or version-specific behavior.

## Rules

- Use `discover.sh` before claiming any symbol, file, or code path is live or dead.
- Use `definition` before assuming which file a class comes from — duplicate names exist.
- Use `is-live` or `live-files` before recommending changes to a file — it may be dead code.
- Use `usages --live-only` instead of grep when finding who calls something.
- Never treat a grep match as reachability evidence.
