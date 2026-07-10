# Crystal — ast-grep Usage Guide

## What discover can answer for Crystal

Crystal's `require` directive handles relative paths, globs, parent-relative paths, shards, and the stdlib — all with the same keyword. A grep hit for a symbol doesn't tell you:

- Whether the file containing it is reachable from your entry point or lives in an unused shard
- Whether a glob `require "./models/*"` actually pulls in that file
- Whether `require "json"` refers to the stdlib or a shard of the same name
- Which files a recursive glob `require "./services/**"` expands to

discover resolves the real require graph and answers these questions structurally.

Crystal also has directory-based requires: `require "./foo"` will look for `foo.cr` or `foo/foo.cr`. discover handles both resolution strategies.

## Pattern syntax

ast-grep patterns are code fragments with placeholder variables. You write real Crystal syntax with metavariables where you want flexible matching.

### Metavariables

| Syntax | Meaning | Example |
|--------|---------|---------|
| `$NAME` | Match exactly one AST node (expression, identifier, type, etc.) | `puts($ARG)` matches `puts(123)` and `puts("hello")` |
| `$$$ARGS` | Match zero or more nodes (variadic) | `puts($$$ARGS)` matches `puts()`, `puts(1)`, `puts(1, 2, 3)` |
| `$$ARGS` | Match one or more nodes | `puts($$ARGS)` matches `puts(1)` and `puts(1, 2)` but not `puts()` |

Metavariable names are arbitrary — `$A`, `$FOO`, `$MY_VAR` all work. Using the same name twice in a pattern means both positions must match the same text.

### How patterns parse in Crystal

Crystal has two lexically distinct name classes: identifiers start lowercase, constants start uppercase. ast-grep handles this automatically — when a metavariable like `$NAME` appears in a position that expects a constant (after `class`, `module`, `struct`, `enum`), the internal expando character is switched from lowercase to uppercase so the pattern parses correctly. You always write `$NAME` regardless of position.

Multiline patterns in Crystal require `\nend` to close blocks:

```bash
# Class with metavar body
ast-grep run -l crystal -p 'class $NAME
  $$$BODY
end' file.cr

# Method definition
ast-grep run -l crystal -p 'def $NAME($$$PARAMS)
end' file.cr
```

Expression-level patterns work directly:

```bash
ast-grep run -l crystal -p 'puts($A)' file.cr
ast-grep run -l crystal -p '$A = $B' file.cr
ast-grep run -l crystal -p '"$A"' file.cr
```

### The `--kind` flag

`--kind` matches by tree-sitter AST node kind instead of by code pattern. It finds every node of that kind in the file regardless of shape.

```bash
ast-grep run -l crystal --kind class_def file.cr --json
```

### Combining `-p` and `--kind`

You cannot use both in the same command. Use `-p` for structural matching (specific shape), `--kind` for exhaustive enumeration (every node of a type).

## Real patterns for common Crystal questions

### Find all classes that inherit from a specific parent
```bash
ast-grep run -l crystal -p 'class $NAME < Exception
  $$$BODY
end' src/
```

### Find all module definitions
```bash
ast-grep run -l crystal -p 'module $NAME
  $$$BODY
end' src/
```

### Find all method calls on a specific receiver
```bash
ast-grep run -l crystal -p 'App.new($$$ARGS)' src/
ast-grep run -l crystal -p 'Log.info { $$$MSG }' src/
```

### Find all requires of a specific shard
```bash
ast-grep run -l crystal -p 'require "json"' src/
ast-grep run -l crystal -p 'require "http/client"' src/
```

### Find all assignments
```bash
ast-grep run -l crystal -p '$A = $B' src/
```

### Find all method definitions with specific parameter shapes
```bash
ast-grep run -l crystal -p 'def $NAME($$$PARAMS)
end' src/
```

### Find all self methods (class-level methods)
```bash
ast-grep run -l crystal -p 'def self.$NAME
end' src/
```

### Find scoped constant access
```bash
ast-grep run -l crystal -p 'Config::$NAME' src/
```

### Find all macro definitions
```bash
ast-grep run -l crystal --kind macro_def src/ --json
```

### Find all struct definitions
```bash
ast-grep run -l crystal -p 'struct $NAME
  $$$BODY
end' src/
```

## AST node kinds used by entity extraction

These are the tree-sitter node kinds that discover's `ast_grep_entities` searches for in Crystal files.

| Node kind | What it matches | Example first line |
|-----------|----------------|-------------------|
| `class_def` | `class Foo`, `class Foo < Bar` | `class Server` |
| `module_def` | `module Foo` | `module MyApp` |
| `struct_def` | `struct Foo` | `struct Config` |
| `enum_def` | `enum Foo` | `enum Status` |
| `method_def` | `def foo`, `def self.foo`, `def initialize` | `def start`, `def self.default` |
| `macro_def` | `macro foo(...)` | `macro define_method(name, content)` |

Entity extraction reports these with simplified kind labels: `class`, `module`, `struct`, `enum`, `def`, `macro`.

## Import/linking model

Crystal connects files through a single `require` directive with multiple resolution strategies determined by the path form.

### Relative requires

```crystal
require "./config"       # resolves to config.cr relative to this file
require "./models/*"     # glob: all .cr files in the models directory
require "./services/**"  # recursive glob: all .cr files in services and subdirectories
require "../utils"       # parent-relative: resolves to utils.cr one directory up
```

Relative requires also support directory-based resolution: `require "./foo"` checks for `foo.cr` first, then `foo/foo.cr`.

### Shard requires

```crystal
require "kemal"          # resolves to lib/kemal/src/kemal.cr
require "db/pool"        # sub-path: resolves to lib/db/src/db/pool.cr
```

Shards are installed into `lib/` by the `shards` package manager, configured in `shard.yml`.

### Stdlib requires

```crystal
require "json"           # could be stdlib or shard — stdlib is not resolvable locally
require "http/client"    # stdlib sub-module
```

discover cannot distinguish stdlib from shards by name alone. If the shard exists in `lib/`, it resolves there. Otherwise the require is treated as external (stdlib).

### Resolution rules

| Path form | Resolves to | discover handles? |
|-----------|------------|-------------------|
| `./path` | `path.cr` or `path/path.cr` relative to requiring file | Yes |
| `./path/*` | All `.cr` files in directory (single level) | Yes |
| `./path/**` | All `.cr` files in directory (recursive) | Yes |
| `../path` | Relative upward from requiring file | Yes |
| `shard_name` | `lib/shard_name/src/shard_name.cr` | Yes (if shard installed) |
| `shard/sub` | `lib/shard/src/shard/sub.cr` | Yes (if shard installed) |
| stdlib name | Stdlib — not resolvable locally | No (external) |
