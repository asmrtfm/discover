# Bash — ast-grep Usage Guide

## What discover can answer for Bash

Bash connects files through `source` and `.` (dot) commands, which resolve relative to the sourcing file, the repo root, or `$PATH`. A grep hit for a function name doesn't tell you:

- Whether the file defining it is actually sourced in your execution path
- Which `source` chain brings that function into scope
- Whether a function is defined in a file you source directly or transitively
- Whether two files source the same dependency (and which version wins)

Bash has no package system or module namespace — all sourced functions share a single global scope. discover tracks which files load which, so you can trace where a function actually comes from.

## Pattern syntax

ast-grep patterns are code fragments with placeholder variables. You write real Bash syntax with metavariables where you want flexible matching.

### Metavariables

| Syntax | Meaning | Example |
|--------|---------|---------|
| `$NAME` | Match exactly one AST node (expression, identifier, command, etc.) | `echo $ARG` matches `echo "hello"` and `echo 42` |
| `$$$ARGS` | Match zero or more nodes (variadic) | `echo $$$ARGS` matches `echo`, `echo a`, `echo a b c` |
| `$$ARGS` | Match one or more nodes | `echo $$ARGS` matches `echo a` and `echo a b` but not `echo` |

Metavariable names are arbitrary — `$A`, `$FOO`, `$MY_VAR` all work. Using the same name twice in a pattern means both positions must match the same text.

### How patterns parse in Bash

Bash uses `impl_lang!` in ast-grep, meaning `$` is a valid identifier character in the grammar. Metavariables like `$NAME` parse natively as Bash variables — no expando substitution is needed. This is the simplest pattern model of any supported language.

Patterns are plain Bash syntax fragments:

```bash
# Function definitions — all three forms
ast-grep run -l bash -p '$FUNC() { $$$BODY }' file.sh
ast-grep run -l bash -p 'function $FUNC { $$$BODY }' file.sh
ast-grep run -l bash -p 'function $FUNC() { $$$BODY }' file.sh

# Commands
ast-grep run -l bash -p 'echo $$$ARGS' file.sh
ast-grep run -l bash -p 'source $PATH' file.sh
```

### The `--kind` flag

`--kind` matches by tree-sitter AST node kind instead of by code pattern. It finds every node of that kind in the file regardless of shape.

```bash
ast-grep run -l bash --kind function_definition file.sh --json
```

### Combining `-p` and `--kind`

You cannot use both in the same command. Use `-p` for structural matching (specific shape), `--kind` for exhaustive enumeration (every node of a type).

## Real patterns for common Bash questions

### Find all function definitions
```bash
# POSIX style
ast-grep run -l bash -p '$NAME() { $$$BODY }' scripts/

# Keyword style
ast-grep run -l bash -p 'function $NAME { $$$BODY }' scripts/

# All forms via --kind
ast-grep run -l bash --kind function_definition scripts/ --json
```

### Find all source/dot directives
```bash
ast-grep run -l bash -p 'source $URI' scripts/
ast-grep run -l bash -p '. $URI' scripts/
```

### Find all trap statements
```bash
ast-grep run -l bash -p "trap '$HANDLER' $SIGNAL" scripts/
```

### Find all local variable declarations
```bash
ast-grep run -l bash -p 'local $NAME=$VALUE' scripts/
```

### Find all export statements
```bash
ast-grep run -l bash -p 'export $NAME=$VALUE' scripts/
```

### Find all if-then blocks
```bash
ast-grep run -l bash --kind if_statement scripts/ --json
```

### Find all case statements
```bash
ast-grep run -l bash --kind case_statement scripts/ --json
```

### Find all command substitutions
```bash
ast-grep run -l bash --kind command_substitution scripts/ --json
```

### Find all while loops
```bash
ast-grep run -l bash --kind while_statement scripts/ --json
```

### Find all for loops
```bash
ast-grep run -l bash --kind for_statement scripts/ --json
```

## AST node kinds used by entity extraction

Bash has a simple entity model — discover only extracts functions. All three function declaration forms produce the same tree-sitter node kind.

| Node kind | What it matches | Example first line |
|-----------|----------------|-------------------|
| `function_definition` | `name() {}`, `function name {}`, `function name() {}` | `process_file() {`, `function my_keyword_func {`, `function my_both_func() {` |

Entity extraction reports all functions with kind `function`. The function name is parsed from the first line of the node text, handling both the `function` keyword prefix and the `()` suffix.

## Import/linking model

Bash connects files through two equivalent directives: `source` and `.` (dot). Both execute the target script in the current shell, making all its variables and functions available in the caller's scope.

### `source`

```bash
source ./lib/utils.sh          # relative to sourcing file
source "./lib/helpers.sh"      # quoted relative path
source /usr/local/lib/app.sh   # absolute path
```

### `.` (dot)

```bash
. ./lib/constants.sh           # identical behavior to source
. "./lib/config.sh"            # quoted path
```

The dot command is the POSIX-standard form; `source` is a Bash extension. Both are functionally identical and discover treats them equivalently.

### Variable-expanded paths

Source paths often use variables or subshells to locate files relative to the script's own directory:

```bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/utils.sh"
source "${SCRIPT_DIR}/lib/config.sh"
. "$(dirname "$0")/lib/init.sh"
```

discover handles these with best-effort resolution: it strips the variable or subshell prefix (`$VAR/`, `${VAR}/`, `$(cmd)/`) and resolves the remaining path suffix relative to the sourcing file, the repo root, and the configured source directory.

### Resolution rules

Paths resolve in this order:

1. **Variable stripping** (if path contains `$`) — strip `$VAR/`, `${VAR}/`, or `$(cmd)/` prefix to extract the path suffix
2. **Relative to the sourcing file** — `./lib/utils.sh` from `scripts/main.sh` resolves to `scripts/lib/utils.sh`
3. **Relative to the repo root** — fallback if the sourcing-file-relative path doesn't exist
4. **Relative to `SRC_DIR`** — further fallback using the configured source directory

| Path form | Resolves to | discover handles? |
|-----------|------------|-------------------|
| `./path.sh` | Relative to sourcing file | Yes |
| `../path.sh` | Parent-relative to sourcing file | Yes |
| `/absolute/path.sh` | Absolute filesystem path | Yes |
| `$VAR/path.sh` | Strip variable, resolve path suffix | Yes (best-effort) |
| `${VAR}/path.sh` | Strip variable, resolve path suffix | Yes (best-effort) |
| `$(cmd)/path.sh` | Strip subshell, resolve path suffix | Yes (best-effort) |
| `bare.sh` | Repo root, then SRC_DIR | Yes |
