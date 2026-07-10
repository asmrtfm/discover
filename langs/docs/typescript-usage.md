# TypeScript — ast-grep Usage Guide

## What discover can answer for TypeScript

TypeScript's import resolution involves extensionless specifiers (`./foo` could be `foo.ts`, `foo.tsx`, or `foo/index.ts`), barrel re-exports (`export * from`), path aliases (`@/`, `~/` via tsconfig `paths`), and the distinction between local modules and `node_modules`. A grep hit for a symbol doesn't tell you:

- Whether the file containing it is actually reachable from your entry point
- Which definition a barrel re-export chain ultimately resolves to
- Whether an import is a local module or an external package
- Whether a `type`-only import creates a runtime dependency or just a compile-time one
- Which of `.ts`, `.tsx`, or `/index.ts` an extensionless specifier resolves to

discover resolves the real import graph and answers these questions structurally.

TypeScript projects also have declaration files (`*.d.ts`) and test files (`*.spec.ts`, `*.test.ts`, `*.spec.tsx`, `*.test.tsx`). discover excludes these by default — they are not part of your source graph.

discover handles both `.ts` and `.tsx` files transparently. ast-grep uses separate grammars for each (`-l typescript` vs `-l tsx`), but discover detects the extension and selects the right grammar automatically — imports, entities, and usages all work on `.tsx` files without any configuration.

## Pattern syntax

ast-grep patterns are code fragments with placeholder variables. You write real TypeScript syntax with metavariables where you want flexible matching.

### Metavariables

| Syntax | Meaning | Example |
|--------|---------|---------|
| `$NAME` | Match exactly one AST node (expression, identifier, type, etc.) | `console.log($ARG)` matches `console.log(123)` and `console.log("hello")` |
| `$$$ARGS` | Match zero or more nodes (variadic) | `foo($$$ARGS)` matches `foo()`, `foo(1)`, `foo(1, 2, 3)` |
| `$$ARGS` | Match one or more nodes | `foo($$ARGS)` matches `foo(1)` and `foo(1, 2)` but not `foo()` |

Metavariable names are arbitrary — `$A`, `$FOO`, `$MY_VAR` all work. Using the same name twice in a pattern means both positions must match the same text.

### How patterns parse in TypeScript

TypeScript uses `impl_lang!` in ast-grep, meaning `$` is a valid identifier character in the grammar. Patterns parse naturally — expressions, statements, and declarations all work at the top level without any wrapping heuristic.

```bash
# Expression patterns
ast-grep run -l typescript -p 'console.log($A)' src/
ast-grep run -l typescript -p 'await $EXPR' src/

# Statement patterns
ast-grep run -l typescript -p 'return $EXPR' src/
ast-grep run -l typescript -p 'throw new $ERR($$$ARGS)' src/

# Declaration patterns
ast-grep run -l typescript -p 'interface $NAME { $$$BODY }' src/
ast-grep run -l typescript -p 'class $NAME extends $PARENT { $$$BODY }' src/
```

### `.ts` vs `.tsx` — choosing the right `-l` flag

ast-grep uses separate grammars: `-l typescript` only matches `.ts` files, `-l tsx` only matches `.tsx` files. When running ast-grep directly, choose accordingly:

```bash
# .ts files only
ast-grep run -l typescript -p 'interface $NAME { $$$BODY }' src/

# .tsx files only (JSX components)
ast-grep run -l tsx -p '<$COMP $$$PROPS />' src/

# Both — run twice
ast-grep run -l typescript -p 'useState($$$A)' src/
ast-grep run -l tsx -p 'useState($$$A)' src/
```

discover's `usages` subcommand handles this automatically — it scans with both grammars and merges the results. Prefer `discover.sh usages` over raw ast-grep when you want hits from both `.ts` and `.tsx` files.

### The `--kind` flag

`--kind` matches by tree-sitter AST node kind instead of by code pattern. It finds every node of that kind in the file regardless of shape.

```bash
ast-grep run -l typescript --kind class_declaration file.ts --json
```

### Combining `-p` and `--kind`

You cannot use both in the same command. Use `-p` for structural matching (specific shape), `--kind` for exhaustive enumeration (every node of a type).

## Real patterns for common TypeScript questions

### Find all instantiations of a class
```bash
ast-grep run -l typescript -p 'new $CLASS($$$ARGS)' src/
```

### Find all async functions returning a specific type
```bash
ast-grep run -l typescript -p 'async function $NAME($$$PARAMS): Promise<$RET> { $$$BODY }' src/
```

### Find all React component definitions (class-based)
```bash
ast-grep run -l typescript -p 'class $NAME extends Component { $$$BODY }' src/
ast-grep run -l typescript -p 'class $NAME extends React.Component { $$$BODY }' src/
```

### Find all imports from a specific module
```bash
ast-grep run -l typescript -p 'import { $$$NAMES } from "react"' src/
ast-grep run -l typescript -p 'import $DEFAULT from "express"' src/
```

### Find all type-only imports
```bash
ast-grep run -l typescript -p 'import type { $$$NAMES } from "$URI"' src/
```

### Find all throw statements
```bash
ast-grep run -l typescript -p 'throw new $ERR($$$ARGS)' src/
```

### Find all await expressions
```bash
ast-grep run -l typescript -p 'await $EXPR' src/
```

### Find all arrow functions assigned to const
```bash
ast-grep run -l typescript -p 'const $NAME = ($$$PARAMS) => $BODY' src/
ast-grep run -l typescript -p 'const $NAME = ($$$PARAMS): $RET => $BODY' src/
```

### Find all interface declarations extending another
```bash
ast-grep run -l typescript -p 'interface $NAME extends $PARENT { $$$BODY }' src/
```

### Find all enum members
```bash
ast-grep run -l typescript -p 'enum $NAME { $$$VALUES }' src/
```

### Find all generic function declarations
```bash
ast-grep run -l typescript -p 'function $NAME<$T>($$$PARAMS): $RET { $$$BODY }' src/
```

### Find all exported default classes
```bash
ast-grep run -l typescript -p 'export default class $NAME { $$$BODY }' src/
```

## AST node kinds used by entity extraction

These are the tree-sitter node kinds that discover's `ast_grep_entities` searches for in TypeScript files. The grammar uses distinct node kinds for each construct.

| Node kind | What it matches | Example first line |
|-----------|----------------|-------------------|
| `interface_declaration` | `interface Foo { ... }` | `interface Greeter {` |
| `type_alias_declaration` | `type Foo = ...` | `type Result<T> = { ok: true; value: T } \| { ok: false; error: Error };` |
| `enum_declaration` | `enum Foo { ... }` | `enum Direction {` |
| `class_declaration` | `class Foo { ... }`, `class Foo extends Bar { ... }` | `class Animal {` |
| `abstract_class_declaration` | `abstract class Foo { ... }` | `abstract class Shape {` |
| `function_declaration` | `function foo() { ... }`, `async function foo() { ... }` | `function greet(name: string): string {` |
| `lexical_declaration` | `const x = ...`, `let x = ...` — discover filters to column 0 or `export`-prefixed for top-level only | `const MAX_RETRIES = 3;` |
| `internal_module` | `namespace Foo { ... }`, `module Foo { ... }` | `namespace MyNamespace {` |

Note: `lexical_declaration` captures all `const`/`let` bindings at every nesting level. discover uses `column == 0` or an `export` prefix to limit to top-level declarations and exclude local variables.

## Import/linking model

TypeScript connects files through `import` and `export` directives. Both contribute to the dependency graph.

### `import`

Brings names from another module into scope.

```typescript
import { readFile } from "fs";                    // external package — unresolvable
import * as path from "path";                     // namespace import — external
import React from "react";                        // default import — external
import { UserService } from "./services/user";    // relative — resolves to ./services/user.ts or ./services/user/index.ts
import type { Config } from "./config";           // type-only — still a file dependency
import { type User, createUser } from "./users";  // inline type — mixed runtime and type dependency
```

### `export` (re-export)

Re-exports another module's names as part of this module's public API. Creates barrel files.

```typescript
export { helper } from "./helpers";              // named re-export
export * from "./utils";                         // barrel re-export — re-exports everything
export type { Options } from "./options";        // type-only re-export
```

discover follows `export` re-exports the same way it follows `import` — they are part of the file dependency graph.

### Resolution rules

| Specifier form | Resolves to | discover handles? |
|----------------|------------|-------------------|
| `"./foo"`, `"../foo"` | `foo.ts`, `foo.tsx`, `foo/index.ts`, or `foo/index.tsx` | Yes |
| `"./foo.js"` | `foo.ts` (TypeScript emits `.js` extensions in output) | Yes (strips `.js`/`.mjs`/`.cjs`) |
| `"react"`, `"express"` | `node_modules/` — external package | No (external) |
| `"@scope/package"` | `node_modules/` — scoped external package | No (external) |

### Extension resolution order

When a specifier like `"./foo"` has no extension, discover tries in order:

1. `foo.ts`
2. `foo.tsx`
3. `foo/index.ts`
4. `foo/index.tsx`

If the specifier already has a `.js`, `.mjs`, or `.cjs` extension, discover strips it before trying the `.ts`/`.tsx` candidates — this handles TypeScript's convention of emitting `.js` extensions in ES module output.
