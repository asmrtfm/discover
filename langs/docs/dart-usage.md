# Dart — ast-grep Usage Guide

## What discover can answer for Dart

Dart has three linking directives (`import`, `export`, `part`) and two URI schemes (`package:` for package-qualified paths, bare relative paths like `../foo.dart`). A grep hit for a symbol doesn't tell you:

- Whether the file containing it is actually reachable from your entry point
- Which definition wins when the same name appears across barrel exports
- Whether a file is pulled in via `import`, re-exported via `export`, or included via `part`
- Whether an import is your own package (`package:my_app/...` → `lib/...`) or an external dependency

discover resolves the real import graph and answers these questions structurally.

Dart projects also generate code (`*.g.dart`, `*.freezed.dart`). discover excludes these by default — they are not part of your source graph.

## Pattern syntax

ast-grep patterns are code fragments with placeholder variables. You write real Dart syntax with metavariables where you want flexible matching.

### Metavariables

| Syntax | Meaning | Example |
|--------|---------|---------|
| `$NAME` | Match exactly one AST node (expression, identifier, type, etc.) | `print($ARG)` matches `print(123)` and `print("hello")` |
| `$$$ARGS` | Match zero or more nodes (variadic) | `print($$$ARGS)` matches `print()`, `print(1)`, `print(1, 2, 3)` |
| `$$ARGS` | Match one or more nodes | `print($$ARGS)` matches `print(1)` and `print(1, 2)` but not `print()` |

Metavariable names are arbitrary — `$A`, `$FOO`, `$MY_VAR` all work. Using the same name twice in a pattern means both positions must match the same text.

### How patterns parse in Dart

Dart's tree-sitter grammar only accepts declarations at the top level. Expression-level patterns (function calls, member access, assignments) would fail to parse as bare source. ast-grep handles this automatically — it wraps expression patterns inside a function body internally, so you can write patterns naturally:

```bash
# These all work despite not being valid top-level Dart
ast-grep run -l dart -p 'print($A)' file.dart
ast-grep run -l dart -p '$X.add($A)' file.dart
ast-grep run -l dart -p 'return $A' file.dart
ast-grep run -l dart -p 'await $EXPR' file.dart
```

Class member patterns (starting with `static`, `final`, etc.) are wrapped in a class body instead:

```bash
ast-grep run -l dart -p 'static String $FIELD = $VALUE' file.dart
ast-grep run -l dart -p 'static final String $FIELD = $VALUE' file.dart
```

Top-level declarations parse directly without wrapping:

```bash
ast-grep run -l dart -p 'class $NAME extends $PARENT { $$$BODY }' file.dart
ast-grep run -l dart -p 'enum $NAME { $$$VALUES }' file.dart
ast-grep run -l dart -p 'void $NAME($$$PARAMS) { $$$BODY }' file.dart
```

### The `--kind` flag

`--kind` matches by tree-sitter AST node kind instead of by code pattern. It finds every node of that kind in the file regardless of shape.

```bash
ast-grep run -l dart --kind class_declaration file.dart --json
```

### Combining `-p` and `--kind`

You cannot use both in the same command. Use `-p` for structural matching (specific shape), `--kind` for exhaustive enumeration (every node of a type).

## Real patterns for common Dart questions

### Find all imports of a specific package
```bash
ast-grep run -l dart -p "import 'package:flutter/$PATH'" lib/ --json
```

### Find all classes that extend a specific parent
```bash
ast-grep run -l dart -p 'class $NAME extends StatefulWidget { $$$BODY }' lib/
ast-grep run -l dart -p 'class $NAME extends StatelessWidget { $$$BODY }' lib/
```

### Find all Navigator.push calls
```bash
ast-grep run -l dart -p 'Navigator.push($$$ARGS)' lib/
ast-grep run -l dart -p 'Navigator.of($CTX).push($$$ARGS)' lib/
```

### Find all setState calls
```bash
ast-grep run -l dart -p 'setState(() { $$$BODY })' lib/
```

### Find all usages of a named constructor
```bash
ast-grep run -l dart -p 'MyWidget.fromJson($$$ARGS)' lib/
```

### Find all await expressions
```bash
ast-grep run -l dart -p 'await $EXPR' lib/
```

### Find all return statements
```bash
ast-grep run -l dart -p 'return $EXPR' lib/
```

### Find if-statements checking a specific condition shape
```bash
ast-grep run -l dart -p 'if ($COND) { $$$BODY }' lib/
```

### Find all member access on a specific class
```bash
ast-grep run -l dart -p 'AppIcons.$FIELD' lib/
```

### Find all async functions
```bash
ast-grep run -l dart -p 'Future<$RET> $NAME($$$PARAMS) async { $$$BODY }' lib/
```

## AST node kinds used by entity extraction

These are the tree-sitter node kinds that discover's `ast_grep_entities` searches for in Dart files. Each has a separate name in the grammar.

| Node kind | What it matches | Example first line |
|-----------|----------------|-------------------|
| `class_declaration` | `class Foo {}`, `class Foo extends Bar {}`, `class Foo with Mixin {}` | `class UserProfile extends StatefulWidget {` |
| `enum_declaration` | `enum Foo { a, b, c }` | `enum UserRole {` |
| `mixin_declaration` | `mixin Foo {}`, `mixin Foo on Bar {}` | `mixin Serializable {` |
| `extension_declaration` | `extension Foo on Bar {}` | `extension StringUtils on String {` |
| `type_alias` | `typedef Foo = Bar`, `typedef Foo = void Function(X)` | `typedef JsonMap = Map<String, dynamic>;` |
| `function_signature` | Function signatures — discover filters to column 0 for top-level only | `void main()`, `Future<UserProfile> fetchUser(String id)` |

Note: `function_signature` captures method signatures at all nesting levels. discover uses `column == 0` to limit to top-level functions and exclude class methods.

## Import/linking model

Dart connects files through three directives, each with different semantics:

### `import`

Brings another library's public namespace into scope.

```dart
import 'dart:io';                              // stdlib — unresolvable (external)
import 'package:flutter/material.dart';        // external package — unresolvable
import 'package:my_app/models/user.dart';      // self-package — resolves to lib/models/user.dart
import '../widgets/button.dart';               // relative — resolves from importing file's directory
import './helpers.dart';                        // relative — resolves from importing file's directory
```

Self-package imports (`package:<your_package>/...`) resolve to `lib/...`. The package name comes from `pubspec.yaml`.

### `export`

Re-exports another library's public API as part of this library's API. Creates barrel files.

```dart
export 'package:my_app/base/nav.dart';         // self-package re-export
export '../widgets/overlay.dart';              // relative re-export
```

discover follows `export` directives the same way it follows `import` — they are part of the file dependency graph.

### `part`

Includes another file as part of the same library. The part file sees all of the library's private members.

```dart
part 'user.g.dart';                            // typically codegen output
```

Part files (especially `*.g.dart` and `*.freezed.dart`) are excluded from discover's file scanning by default since they are generated code, not source you navigate.

### Resolution rules

| URI form | Resolves to | discover handles? |
|----------|------------|-------------------|
| `dart:*` | Dart stdlib | No (external) |
| `package:<your_package>/*` | `lib/*` | Yes |
| `package:<other>/*` | External dependency | No (external) |
| `../path.dart`, `./path.dart` | Relative to importing file | Yes |
