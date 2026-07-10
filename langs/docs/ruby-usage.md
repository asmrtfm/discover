# Ruby — ast-grep Usage Guide

## What discover can answer for Ruby

Ruby connects files through `require` and `require_relative`, which resolve differently. `require` searches `$LOAD_PATH` — it could hit a gem's file, a stdlib file, or your own `lib/` directory, and grep can't tell which. `require_relative` resolves relative to the requiring file, but following a chain of relative requires across directories manually is tedious and error-prone.

discover resolves the real require graph and answers these questions structurally:

- Which files are actually reachable from your entry point through require chains
- Whether a `require "foo"` refers to your own `lib/foo.rb` or an external gem
- What modules, classes, and methods are defined in files that a given file depends on
- Which files in the project use (require) a given file

## Pattern syntax

ast-grep patterns are code fragments with placeholder variables. You write real Ruby syntax with metavariables where you want flexible matching.

### Metavariables

| Syntax | Meaning | Example |
|--------|---------|---------|
| `$NAME` | Match exactly one AST node (expression, identifier, type, etc.) | `puts($ARG)` matches `puts(123)` and `puts("hello")` |
| `$$$ARGS` | Match zero or more nodes (variadic) | `MyClass.new($$$ARGS)` matches `MyClass.new`, `MyClass.new(1)`, `MyClass.new(1, 2)` |
| `$$ARGS` | Match one or more nodes | `MyClass.new($$ARGS)` matches `MyClass.new(1)` but not `MyClass.new` |

Metavariable names are arbitrary — `$A`, `$FOO`, `$MY_VAR` all work. Using the same name twice in a pattern means both positions must match the same text.

### How patterns parse in Ruby

Ruby uses the `impl_lang_expando!` macro in ast-grep. The `$` character is not a valid Ruby identifier character (it is a sigil for global variables like `$stdout`, but metavariable names like `$NAME` are uppercase identifiers that don't collide). ast-grep internally substitutes `$` with an expando character (`µ`) before parsing. You still write `$VAR` in your patterns — the substitution is transparent.

Ruby's tree-sitter grammar accepts both expression-level and declaration-level patterns at the top level, so patterns parse directly without internal wrapping:

```bash
# Expression patterns
ast-grep run -l ruby -p 'puts($A)' lib/
ast-grep run -l ruby -p '$X.each { |$ITEM| $$$BODY }' lib/
ast-grep run -l ruby -p 'return $EXPR' lib/

# Declaration patterns
ast-grep run -l ruby -p 'class $NAME < $PARENT; $$$BODY; end' lib/
ast-grep run -l ruby -p 'module $NAME; $$$BODY; end' lib/
ast-grep run -l ruby -p 'def $METHOD($$$PARAMS); $$$BODY; end' lib/
```

### The `--kind` flag

`--kind` matches by tree-sitter AST node kind instead of by code pattern. It finds every node of that kind in the file regardless of shape.

```bash
ast-grep run -l ruby --kind class file.rb --json
ast-grep run -l ruby --kind method file.rb --json
```

### Combining `-p` and `--kind`

You cannot use both in the same command. Use `-p` for structural matching (specific shape), `--kind` for exhaustive enumeration (every node of a type).

## Real patterns for common Ruby questions

### Find all classes that inherit from a specific parent
```bash
# Plain Ruby
ast-grep run -l ruby -p 'class $NAME < $PARENT; $$$BODY; end' lib/

# Rails — models, controllers, jobs live under app/
ast-grep run -l ruby -p 'class $NAME < ApplicationRecord; $$$BODY; end' app/models/
ast-grep run -l ruby -p 'class $NAME < ApplicationController; $$$BODY; end' app/controllers/
ast-grep run -l ruby -p 'class $NAME < ApplicationJob; $$$BODY; end' app/jobs/
```

### Find all method definitions
```bash
ast-grep run -l ruby -p 'def $METHOD($$$PARAMS)' lib/
ast-grep run -l ruby -p 'def self.$METHOD($$$PARAMS)' lib/
```

### Find all usages of a class constructor
```bash
ast-grep run -l ruby -p 'MyClass.new($$$ARGS)' lib/
ast-grep run -l ruby -p 'User.new($$$ARGS)' app/
```

### Find all include/extend/prepend declarations
```bash
# In a gem
ast-grep run -l ruby -p 'include $MODULE' lib/

# In Rails — concerns live under app/models/concerns/ and app/controllers/concerns/
ast-grep run -l ruby -p 'include $MODULE' app/models/
ast-grep run -l ruby -p 'extend $MODULE' app/models/
ast-grep run -l ruby -p 'prepend $MODULE' app/models/
```

### Find all attr_reader/attr_writer/attr_accessor declarations
```bash
ast-grep run -l ruby -p 'attr_reader $$$ATTRS' lib/
ast-grep run -l ruby -p 'attr_accessor $$$ATTRS' app/models/
```

### Find all blocks with a specific method
```bash
ast-grep run -l ruby -p '$X.map { |$ITEM| $$$BODY }' lib/
ast-grep run -l ruby -p '$X.select { |$ITEM| $$$BODY }' lib/
ast-grep run -l ruby -p '$X.each_with_object($INIT) { |$ARGS| $$$BODY }' lib/
```

### Find all rescue blocks
```bash
ast-grep run -l ruby -p 'rescue $EXCEPTION => $VAR' lib/
```

### Find all raise statements
```bash
ast-grep run -l ruby -p 'raise $EXCEPTION' lib/
ast-grep run -l ruby -p 'raise $EXCEPTION, $MSG' lib/
```

### Find all require_relative statements
```bash
ast-grep run -l ruby -p 'require_relative "$PATH"' lib/
```

### Rails-specific patterns

#### Find all callbacks
```bash
ast-grep run -l ruby -p 'before_action $$$ARGS' app/controllers/
ast-grep run -l ruby -p 'after_commit $$$ARGS' app/models/
ast-grep run -l ruby -p 'before_validation $$$ARGS' app/models/
```

#### Find all scope definitions
```bash
ast-grep run -l ruby -p 'scope $$$ARGS' app/models/
```

#### Find all has_many / belongs_to associations
```bash
ast-grep run -l ruby -p 'has_many $$$ARGS' app/models/
ast-grep run -l ruby -p 'belongs_to $$$ARGS' app/models/
ast-grep run -l ruby -p 'has_one $$$ARGS' app/models/
```

#### Find all validations
```bash
ast-grep run -l ruby -p 'validates $$$ARGS' app/models/
```

### Find all module definitions
```bash
ast-grep run -l ruby -p 'module $NAME; $$$BODY; end' lib/
```

## AST node kinds used by entity extraction

These are the tree-sitter node kinds that discover's `ast_grep_entities` searches for in Ruby files. Ruby's grammar uses distinct node kinds for each construct.

| Node kind | What it matches | Discover kind | Example first line |
|-----------|----------------|---------------|-------------------|
| `module` | `module Foo ... end` | `module` | `module MyModule` |
| `class` | `class Foo ... end`, `class Foo < Bar ... end` | `class` | `class TopClass < Base` |
| `method` | `def foo ... end` (instance methods) | `def` | `def my_method` |
| `singleton_method` | `def self.foo ... end` (class methods) | `def self` | `def self.class_method` |

Entity name extraction uses the first line of each match and captures the identifier after the keyword. For methods, names can include `?`, `!`, and `=` suffixes (e.g., `valid?`, `save!`, `name=`).

Note: unlike some languages, discover does not filter by column position for Ruby — all matched nodes are included regardless of nesting depth. A `method` inside a `class` inside a `module` is captured as an entity.

## Import/linking model

Ruby connects files through two require directives with fundamentally different resolution strategies.

### `require`

Searches `$LOAD_PATH` for the specified file. This is how gems and stdlib modules are loaded.

```ruby
require "json"              # stdlib — unresolvable (external)
require "rails"             # gem — unresolvable (external)
require "my_app/models"     # could be your lib/ or a gem — ambiguous
```

For project-local bare requires, discover checks `lib/<specifier>.rb` and `lib/<specifier>/<specifier>.rb` as a best-effort resolution. If the file exists under `lib/`, it resolves; otherwise it's treated as external.

### `require_relative`

Resolves relative to the directory of the file containing the `require_relative` statement. Always resolves to a local file.

```ruby
require_relative "./foo"        # → <current_dir>/foo.rb
require_relative "../bar"       # → <parent_dir>/bar.rb
require_relative "models/user"  # → <current_dir>/models/user.rb
```

discover appends `.rb` if the specifier doesn't already have an extension, then resolves relative to the requiring file's directory.

### Resolution rules

| Require form | Resolves to | discover handles? |
|-------------|------------|-------------------|
| `require "json"` | Stdlib via `$LOAD_PATH` | No (external) |
| `require "my_gem"` | Gem via `$LOAD_PATH` | No (external) |
| `require "foo/bar"` | `lib/foo/bar.rb` if it exists | Best-effort yes |
| `require_relative "./foo"` | Relative to requiring file → `foo.rb` | Yes |
| `require_relative "../bar"` | Relative upward → `bar.rb` | Yes |
| `require_relative "baz"` | Relative to requiring file → `baz.rb` | Yes |

### Directory conventions

Plain Ruby projects (gems, libraries, CLI tools) place source files under `lib/`. discover defaults `SRC_DIR_REL` to `lib` and uses this as the base for bare `require` resolution.

Rails projects use a completely different layout. Source code lives under `app/`, split across convention-driven subdirectories:

| Directory | Contains |
|-----------|----------|
| `app/models/` | ActiveRecord models, concerns, POROs |
| `app/controllers/` | Request handlers |
| `app/views/` | Templates (ERB, Jbuilder, Turbo Stream) |
| `app/services/` | Service objects (app-specific convention) |
| `app/jobs/` | Background jobs |
| `app/mailers/` | Email generators |
| `app/helpers/` | View helpers |
| `lib/` | Non-autoloaded code, Rake tasks, generators |

Rails autoloads everything under `app/` — files don't need `require` or `require_relative` to see each other. The autoloader (`Zeitwerk` in modern Rails) maps file paths to constant names (`app/models/user_profile.rb` → `UserProfile`), so the directory structure *is* the namespace structure. This means discover's static require-graph analysis covers less of the dependency picture in a Rails project than in a plain Ruby project.

For Rails projects, set `SRC_DIR_REL` to `app` when running `discover init`, and consider installing the `discover-rails.sh` companion script, which uses `bin/rails runner` for runtime introspection that static analysis can't provide — module ancestry chains, autoloader resolution, route/namespace mapping, and concern propagation through inheritance and hooks.
