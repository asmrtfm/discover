# Kotlin — ast-grep Usage Guide

## What discover can answer for Kotlin

Kotlin's import system is package-based — `import com.example.Foo` could resolve to any `.kt` file declaring that class in its package directory. A grep hit doesn't tell you:

- Whether the file containing a symbol is actually reachable from your entry point
- Which file provides a symbol when multiple files exist in the same package
- Whether an import is a local module or an external dependency (Maven/Gradle)
- Whether a wildcard import (`com.example.utils.*`) pulls in 2 files or 20

discover resolves the real import graph by mapping packages to source paths and answers these questions structurally.

Kotlin projects may also have script files (`*.kts`) and test files under `src/test/`. discover excludes these by default — they are not part of your source graph.

## Pattern syntax

ast-grep patterns are code fragments with placeholder variables. You write real Kotlin syntax with metavariables where you want flexible matching.

### Metavariables

| Syntax | Meaning | Example |
|--------|---------|---------|
| `$NAME` | Match exactly one AST node (expression, identifier, type, etc.) | `println($ARG)` matches `println(123)` and `println("hello")` |
| `$$$ARGS` | Match zero or more nodes (variadic) | `foo($$$ARGS)` matches `foo()`, `foo(1)`, `foo(1, 2, 3)` |
| `$$ARGS` | Match one or more nodes | `foo($$ARGS)` matches `foo(1)` and `foo(1, 2)` but not `foo()` |

Metavariable names are arbitrary — `$A`, `$FOO`, `$MY_VAR` all work. Using the same name twice in a pattern means both positions must match the same text.

### How patterns parse in Kotlin

Kotlin uses `impl_lang_expando!` in ast-grep with expando char `µ`, meaning `$` is not a valid identifier character in the grammar. The expando substitution makes patterns work transparently — expressions, statements, and declarations all parse at the top level.

```bash
# Expression patterns
ast-grep run -l kotlin -p 'println($A)' src/
ast-grep run -l kotlin -p 'listOf($$$ITEMS)' src/

# Statement patterns
ast-grep run -l kotlin -p 'return $EXPR' src/
ast-grep run -l kotlin -p 'throw $EXCEPTION' src/

# Declaration patterns
ast-grep run -l kotlin -p 'data class $NAME($$$PROPS)' src/
ast-grep run -l kotlin -p 'object $NAME { $$$BODY }' src/
```

### The `--kind` flag

`--kind` matches by tree-sitter AST node kind instead of by code pattern. It finds every node of that kind in the file regardless of shape.

```bash
ast-grep run -l kotlin --kind class_declaration file.kt --json
```

### Combining `-p` and `--kind`

You cannot use both in the same command. Use `-p` for structural matching (specific shape), `--kind` for exhaustive enumeration (every node of a type).

## Real patterns for common Kotlin questions

### Find all data class declarations
```bash
ast-grep run -l kotlin -p 'data class $NAME($$$PROPS)' src/
```

### Find all object singletons
```bash
ast-grep run -l kotlin -p 'object $NAME { $$$BODY }' src/
```

### Find all suspend functions
```bash
ast-grep run -l kotlin -p 'suspend fun $NAME($$$PARAMS): $RET { $$$BODY }' src/
```

### Find all sealed class hierarchies
```bash
ast-grep run -l kotlin -p 'sealed class $NAME { $$$BODY }' src/
```

### Find all extension functions on a type
```bash
ast-grep run -l kotlin -p 'fun String.$NAME($$$PARAMS): $RET { $$$BODY }' src/
```

### Find all coroutine launches
```bash
ast-grep run -l kotlin -p 'launch { $$$BODY }' src/
ast-grep run -l kotlin -p 'async { $$$BODY }' src/
```

### Find all when expressions
```bash
ast-grep run -l kotlin -p 'when ($EXPR) { $$$BRANCHES }' src/
```

### Find all companion object declarations
```bash
ast-grep run -l kotlin -p 'companion object { $$$BODY }' src/
```

### Find all lazy property delegates
```bash
ast-grep run -l kotlin -p 'val $NAME by lazy { $$$BODY }' src/
```

### Find all annotation usages
```bash
ast-grep run -l kotlin -p '@Inject $$$REST' src/
```

### Find all null-safe calls on a type
```bash
ast-grep run -l kotlin -p '$EXPR?.$METHOD($$$ARGS)' src/
```

### Find all require/check assertions
```bash
ast-grep run -l kotlin -p 'require($CONDITION) { $$$MSG }' src/
```

## AST node kinds used by entity extraction

These are the tree-sitter node kinds that discover's `ast_grep_entities` searches for in Kotlin files. Note that Kotlin's tree-sitter grammar groups several constructs under `class_declaration`.

| Node kind | What it matches | Example first line |
|-----------|----------------|-------------------|
| `class_declaration` | `class`, `data class`, `enum class`, `sealed class`, `abstract class`, `annotation class`, `interface` — discover filters to column 0 for top-level only | `data class UserProfile(val name: String)` |
| `object_declaration` | `object Foo { ... }` — top-level singletons only; `companion object` has its own kind | `object AppConfig {` |
| `function_declaration` | `fun foo()`, `suspend fun foo()` — discover filters to column 0 for top-level only | `suspend fun fetchUser(id: String): UserProfile {` |
| `type_alias` | `typealias Foo = ...` | `typealias JsonMap = Map<String, Any>` |
| `property_declaration` | `val x = ...`, `var x = ...` — discover filters to column 0 for top-level only | `val MAX_RETRIES = 3` |

Note: `class_declaration` captures interfaces, enums, sealed classes, data classes, abstract classes, and annotation classes — discover parses the leading keyword to determine the actual kind. Nested classes (column > 0) are excluded.

## Import/linking model

Kotlin connects files through `import` statements. There are no export or re-export directives — all top-level declarations in a file are visible to other files in the same module by default (internal visibility).

### `import`

Brings names from another package into scope.

```kotlin
import kotlin.collections.List                      // stdlib — unresolvable
import com.example.models.User                      // specific class from local package
import com.example.utils.*                           // wildcard — all declarations in package
import com.example.network.ApiClient as Client       // aliased import
import kotlinx.coroutines.launch                     // extension library — unresolvable
```

### Resolution rules

| Specifier form | Resolves to | discover handles? |
|----------------|------------|-------------------|
| `com.example.app.Foo` | `src/main/kotlin/com/example/app/Foo.kt` | Yes (when under PACKAGE_NAME) |
| `com.example.app.*` | All `.kt` files in `src/main/kotlin/com/example/app/` | Yes (when under PACKAGE_NAME) |
| `com.example.Foo as Bar` | Same as `com.example.Foo` (alias stripped) | Yes |
| `kotlin.*`, `kotlinx.*` | Kotlin stdlib/extensions | No (stdlib) |
| `java.*`, `javax.*` | Java stdlib | No (platform) |
| `android.*`, `androidx.*` | Android platform/Jetpack | No (platform) |
| `com.thirdparty.*` | External Maven/Gradle dependency | No (external) |

### Package-to-path mapping

Kotlin follows the Java convention: the dotted package name maps to a directory path under the source root. `com.example.app.models.User` becomes `src/main/kotlin/com/example/app/models/User.kt`.

For Android projects that place Kotlin files in `src/main/java/`, set `SRC_DIR_REL="src/main/java"`. For projects with multiple source roots, set `KOTLIN_EXTRA_SRC_DIRS` to space-separated additional roots.
