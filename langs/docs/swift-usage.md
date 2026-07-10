# Swift — ast-grep Usage Guide

## What discover can answer for Swift

Swift's compilation model is module-based, not file-based. All `.swift` files in the same target/module see each other implicitly — there are no intra-module imports. An `import NetworkKit` statement brings in an entire module, not a single file. This means:

- Grep can't tell you which target owns a file or what module boundary a symbol crosses
- A symbol reference doesn't tell you whether it's same-module (always visible) or cross-module (must be `public`/`open`)
- `import MyModule` resolves to a set of source files, not one file — you need the module graph to know which ones
- Access control (`public`, `internal`, `private`) determines cross-module visibility but grep ignores it entirely

discover maps the module graph (SPM targets, Xcode targets, or single-target fallback) and answers these questions structurally. It parses `Package.swift` via `swift package dump-package` for SPM projects, reads `.pbxproj` for Xcode-only projects, and falls back to treating all files in `Sources/` as one implicit module.

## Pattern syntax

ast-grep patterns are code fragments with placeholder variables. You write real Swift syntax with metavariables where you want flexible matching.

### Metavariables

| Syntax | Meaning | Example |
|--------|---------|---------|
| `$NAME` | Match exactly one AST node (expression, identifier, type, etc.) | `print($ARG)` matches `print(123)` and `print("hello")` |
| `$$$ARGS` | Match zero or more nodes (variadic) | `print($$$ARGS)` matches `print()`, `print(1)`, `print(1, 2, 3)` |
| `$$ARGS` | Match one or more nodes | `print($$ARGS)` matches `print(1)` and `print(1, 2)` but not `print()` |

Metavariable names are arbitrary — `$A`, `$FOO`, `$MY_VAR` all work. Using the same name twice in a pattern means both positions must match the same text.

### How patterns parse in Swift

Swift uses the `impl_lang_expando!` macro in ast-grep. The `$` character is not a valid Swift identifier character, so ast-grep internally substitutes it with an expando character (`µ`) before parsing. You still write `$VAR` in your patterns — the substitution is transparent.

Unlike Dart, Swift's tree-sitter grammar accepts most expression-level patterns at the top level without needing an internal wrapping strategy. Patterns parse directly:

```bash
# Expression patterns work at top level
ast-grep run -l swift -p 'print($A)' Sources/
ast-grep run -l swift -p '$X.fetch($$$ARGS)' Sources/
ast-grep run -l swift -p 'return $EXPR' Sources/
ast-grep run -l swift -p 'await $EXPR' Sources/
```

Declaration patterns also work directly:

```bash
ast-grep run -l swift -p 'struct $NAME { $$$BODY }' Sources/
ast-grep run -l swift -p 'class $NAME: $PARENT { $$$BODY }' Sources/
ast-grep run -l swift -p 'func $NAME($$$PARAMS) -> $RET { $$$BODY }' Sources/
ast-grep run -l swift -p 'protocol $NAME { $$$BODY }' Sources/
```

### The `--kind` flag

`--kind` matches by tree-sitter AST node kind instead of by code pattern. It finds every node of that kind in the file regardless of shape.

```bash
ast-grep run -l swift --kind class_declaration file.swift --json
```

### Combining `-p` and `--kind`

You cannot use both in the same command. Use `-p` for structural matching (specific shape), `--kind` for exhaustive enumeration (every node of a type).

## Real patterns for common Swift questions

### Find all URLSession calls
```bash
ast-grep run -l swift -p 'URLSession.shared.$METHOD($$$ARGS)' Sources/
```

### Find all guard-let unwraps
```bash
ast-grep run -l swift -p 'guard let $VAR = $EXPR else { $$$BODY }' Sources/
```

### Find all if-let unwraps
```bash
ast-grep run -l swift -p 'if let $VAR = $EXPR { $$$BODY }' Sources/
```

### Find all classes conforming to a protocol
```bash
ast-grep run -l swift -p 'class $NAME: Codable { $$$BODY }' Sources/
ast-grep run -l swift -p 'struct $NAME: Codable { $$$BODY }' Sources/
```

### Find all @main entry points
```bash
ast-grep run -l swift -p '@main struct $NAME { $$$BODY }' Sources/
```

### Find all async functions
```bash
ast-grep run -l swift -p 'func $NAME($$$PARAMS) async -> $RET { $$$BODY }' Sources/
ast-grep run -l swift -p 'func $NAME($$$PARAMS) async throws -> $RET { $$$BODY }' Sources/
```

### Find all Task initializations
```bash
ast-grep run -l swift -p 'Task { $$$BODY }' Sources/
ast-grep run -l swift -p 'Task.detached { $$$BODY }' Sources/
```

### Find all property wrappers usage
```bash
ast-grep run -l swift -p '@State var $NAME: $TYPE' Sources/
ast-grep run -l swift -p '@Published var $NAME: $TYPE' Sources/
ast-grep run -l swift -p '@ObservedObject var $NAME: $TYPE' Sources/
```

### Find all switch statements on a specific type
```bash
ast-grep run -l swift -p 'switch $EXPR { $$$CASES }' Sources/
```

### Find all extensions on a specific type
```bash
ast-grep run -l swift -p 'extension String { $$$BODY }' Sources/
```

### Find all try? expressions
```bash
ast-grep run -l swift -p 'try? $EXPR' Sources/
```

### Find all NotificationCenter observers
```bash
ast-grep run -l swift -p 'NotificationCenter.default.addObserver($$$ARGS)' Sources/
```

## AST node kinds used by entity extraction

Swift's tree-sitter grammar reuses `class_declaration` for multiple declaration types. discover dispatches on the leading keyword in the matched text to classify them.

| Node kind | What it matches | Discover kind | Example first line |
|-----------|----------------|---------------|-------------------|
| `class_declaration` | `struct Foo {}` | `struct` | `public struct User: Codable {` |
| `class_declaration` | `class Foo {}` | `class` | `public class APIClient {` |
| `class_declaration` | `enum Foo {}` | `enum` | `enum UserRole {` |
| `class_declaration` | `actor Foo {}` | `actor` | `actor DataStore {` |
| `class_declaration` | `extension Foo {}` | `extension` | `extension String {` |
| `protocol_declaration` | `protocol Foo {}` | `protocol` | `protocol Fetchable {` |
| `function_declaration` | `func foo() {}` | `func` | `func main() {` |

Note: `function_declaration` is filtered to column 0 to limit to top-level functions. Methods nested inside types are excluded from entity extraction.

The `class_declaration` node kind's overloaded nature means `--kind class_declaration` returns structs, enums, classes, actors, and extensions all at once. discover's entity extraction parses the first line of each match to determine the actual kind.

Access control modifiers (`public`, `open`, `internal`, `fileprivate`, `private`, `package`) appear in the signature text and are used by discover's cross-module definition search to filter visibility — only `public` and `open` symbols are visible across module boundaries.

## Import/linking model

Swift connects files through a module system, not a file-level import system. This is fundamentally different from languages like Dart, TypeScript, or Ruby.

### Intra-module: implicit visibility

All `.swift` files in the same target/module see each other automatically. There are no per-file imports needed within a module. If `User.swift` and `UserService.swift` are both in the `Models` target, `UserService` can reference `User` without any import statement.

This means discover doesn't resolve intra-module references via imports — it resolves them by knowing which files belong to the same module.

### Inter-module: `import`

`import` brings an entire module's public API into scope.

```swift
import Foundation           // system framework — unresolvable (external)
import UIKit                // system framework — unresolvable (external)
import NetworkKit           // local module — resolves to all files in Sources/NetworkKit/
import Models               // local module — resolves to all files in Sources/Models/
```

`@testable import` makes `internal` symbols visible in test targets:

```swift
@testable import App        // test-only — resolves same as `import App`
```

discover resolves local module imports to the set of source files in that module's directory. External framework/package imports are not resolvable.

### Module graph sources

discover detects project type and loads the module graph accordingly:

| Project type | Detection | Module graph source |
|-------------|-----------|-------------------|
| SPM | `Package.swift` exists | `swift package dump-package` JSON output |
| Xcode | `.xcodeproj/project.pbxproj` exists, no `Package.swift` | Lightweight `.pbxproj` parsing for `PBXNativeTarget` entries |
| Single-target | Neither found | All `.swift` files in `Sources/` treated as one module |

### Resolution rules

| Import form | Resolves to | discover handles? |
|-------------|------------|-------------------|
| `import Foundation` (system) | System framework | No (external) |
| `import MyLocalModule` | All `.swift` files in the module's source directory | Yes |
| `@testable import MyModule` | Same as above (strips `@testable` prefix) | Yes |
| `import ExternalPackage` | External SPM dependency | No (external) |

### How discover navigates definitions across modules

When searching for a symbol definition, discover follows a four-step search order:

1. **Local file** — check the current file first
2. **Same-module peers** — scan all other `.swift` files in the same module (no access control filtering, since intra-module `internal` visibility applies)
3. **Imported modules** — scan files in modules named in `import` statements (only `public`/`open` symbols)
4. **Transitive dependencies** — scan files in all modules reachable through the dependency graph (only `public`/`open` symbols)
