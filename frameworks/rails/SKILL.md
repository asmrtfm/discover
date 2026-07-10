---
name: discover-rails
description: Runtime Rails introspection that complements discover-ruby. Uses rails runner for questions only the loaded app can answer — module ancestry, runtime dispatch, namespace mapping.
---

## Why this exists

`discover.sh` does static AST analysis — it finds `include Searchable` in source but can't tell you which models inherited a concern through a parent class or auto-include hook, or which models got a module via runtime dispatch. `discover-rails.sh` fills that gap with runtime introspection via `bin/rails runner`.

Use `discover.sh` first for static structure. Escalate to `discover-rails.sh` when the answer depends on runtime state.

## Script location

```
{{SCRIPT_PATH}}
```

Run from the repo root. Requires `bin/rails` and a bootable Rails environment.

## Terminology

- **includes** — broadest sense: any path (direct `include`, inheritance, hooks, runtime dispatch). "Does this class have this module in its ancestors?"
- **imports / requires** — direct only: an explicit `include X` statement in the source file. Delegates to `discover.sh` static analysis.
- **inherits** — indirect only: the class got the module through inheritance, auto-include hooks, or runtime dispatch, but never wrote `include X` itself. This is `includes` minus `imports`.

## Subcommands

### includes

All `ApplicationRecord` descendants that have the module anywhere in their ancestor chain.

```bash
bash {{SCRIPT_PATH}} includes Searchable
bash {{SCRIPT_PATH}} includes ActiveModel::Validations --json
```

Output with `--json` includes class name and table name for each match.

### imports / requires

Classes that directly `include <MODULE>` in their source file. Delegates to `discover.sh imports`.

```bash
bash {{SCRIPT_PATH}} imports Searchable
bash {{SCRIPT_PATH}} requires Searchable
```

### inherits

Classes that have the module in their ancestors but do NOT directly include it — they got it via inheritance, hooks, or runtime dispatch.

```bash
bash {{SCRIPT_PATH}} inherits Searchable
bash {{SCRIPT_PATH}} inherits Searchable --json
```

### namespaces

Map the application's namespace structure — controllers, routes, views, and route files per namespace.

```bash
bash {{SCRIPT_PATH}} namespaces                # summary of all namespaces
bash {{SCRIPT_PATH}} namespaces admin           # full detail for one namespace
bash {{SCRIPT_PATH}} namespaces --json          # summary as JSON
bash {{SCRIPT_PATH}} namespaces api --json      # detail as JSON (includes route list)
```

## Rules

- Use `imports` when you need to know where `include X` is written in source.
- Use `includes` when you need the full runtime picture.
- Use `inherits` when you need to understand indirect propagation paths.
- When a concern dispatches to submodules dynamically, use `includes` with the specific submodule name.
- `namespaces` maps the route-driven namespace structure — use it to understand how controllers, views, and route files are organized across `app/`.
