# ast-grep: field declaration patterns don't match class members

**Date:** 2026-06-05
**Repo:** `~/.src/github/asmrtfm/ast-grep`
**File:** `crates/language/src/dart.rs`

## Problem

Pattern-based matching does not work for class field declarations. This pattern returns zero matches against a file containing exactly that code:

```
ast-grep run -l dart -p 'static String $FIELD = $VALUE' lib/theme/icons.dart --json
# → 0 matches
```

Even the simplest cases fail:

```
echo 'class Foo { String bar = "hello"; }' > /tmp/t.dart
ast-grep run -l dart -p 'String $FIELD = $VALUE' /tmp/t.dart --json
# → 0 matches
```

But local variable declarations inside function bodies work fine:

```
echo 'void f() { String bar = "hello"; }' > /tmp/t.dart
ast-grep run -l dart -p 'String $FIELD = $VALUE' /tmp/t.dart --json
# → 1 match: "String bar = \"hello\";"
```

## Root cause

Tree-sitter-dart produces **different node structures** for class fields vs local variables:

### Local variable (inside function body)
```
local_variable_declaration
  initialized_variable_definition     ← pattern matches this node
    type → type_identifier ("String")
    name: identifier ("bar")
    = 
    value: string_literal ("hello")
  ;
```

### Class field (inside class body)
```
class_member
  declaration                          ← pattern SHOULD match this, but doesn't
    [static]
    type → type_identifier ("String")
    initialized_identifier_list
      initialized_identifier
        name: identifier ("bar")
        =
        value: string_literal ("hello")
  ;
```

The key differences:
1. **Different node kind**: `initialized_variable_definition` (local) vs `declaration` (class field)
2. **Different structure**: In `declaration`, the type and the initialized_identifier are separated by an `initialized_identifier_list` wrapper node
3. **`static` keyword**: Only valid inside class bodies, not function bodies

### Why the current wrapping logic fails

`dart.rs` has a wrapping mechanism (`try_wrapped_pattern`) that takes patterns that don't parse as top-level constructs and wraps them in `void _() { ... }`. This handles expression patterns like `print($A)` and `AppIcons.$FIELD`.

But for field declarations:
- `static String $FIELD = $VALUE` → invalid inside a function body (`static` not allowed), so wrapping fails
- `String $FIELD = $VALUE` → wraps successfully, parses as `initialized_variable_definition` inside `local_variable_declaration`, but this node kind doesn't exist in the class field tree (which uses `declaration` → `initialized_identifier_list` → `initialized_identifier`)

So the pattern parses as one tree shape, the source has a different tree shape, and they never match.

## Fix direction

The `build_pattern` / `try_wrapped_pattern` logic in `dart.rs` needs a **class body wrapper** path in addition to the function body wrapper. When wrapping in `void _() { ... }` fails or produces a node kind that wouldn't match class members, try wrapping in `class _W_ { ... }` instead.

Specifically:
1. Add a class wrapper: `const CLASS_WRAPPER_PREFIX: &str = "class _W_ {\n";` / `const CLASS_WRAPPER_SUFFIX: &str = "\n}";`
2. In `try_wrapped_pattern`, after the function-body attempts fail, try wrapping in a class body
3. When extracting from the class wrapper, navigate to `class_body` → first `class_member` → `declaration` node
4. The extracted `declaration` pattern node should then match `declaration` nodes in real class bodies

The trickiest part: a pattern like `String $FIELD = $VALUE` is valid in BOTH contexts (local var and class field) but produces different node kinds. The fix may need to:
- Try both wrappers and produce the one that matches the target context, OR
- Produce both pattern variants and match against either

### Existing tests to update

`crates/language/src/dart.rs` has tests at the bottom. Add tests for:

```rust
#[test]
fn test_dart_class_field_declaration() {
  test_match("String $FIELD = $VALUE", "class Foo { String bar = \"hello\"; }");
  test_match("static String $FIELD = $VALUE", "class Foo { static String bar = \"hello\"; }");
}
```

### Build and test

```bash
cd ~/.src/github/asmrtfm/ast-grep
cargo test -p ast-grep-language -- dart
cargo build --release
# Binary lands at target/release/ast-grep (or sg)
# Copy to ~/.cargo/bin/ast-grep to install
```

## Impact

This blocks the `--assets` flag in `malsi/scripts/discover.sh`. The flag needs to find which `AppIcons` field holds a given asset path by matching `static String $FIELD = "assets/icons/Foo.svg"` inside the `AppIcons` class. Without this fix, `discover.sh` has to fall back to grep for this step, which defeats the purpose of the tool.

## CST dumps for reference

Pattern `String $FIELD = $VALUE` parses as:
```
top_level_variable_declaration
  type → type_identifier
  initialized_identifier_list
    initialized_identifier
      name: identifier ($FIELD)
      value: identifier ($VALUE)
```

Source `class Foo { String bar = "hello"; }` class field is:
```
class_member
  declaration
    type → type_identifier ("String")
    initialized_identifier_list
      initialized_identifier
        name: identifier ("bar")
        =
        value: string_literal ("hello")
  ;
```

Source `void f() { String bar = "hello"; }` local var is:
```
local_variable_declaration
  initialized_variable_definition
    type → type_identifier ("String")
    name: identifier ("bar")
    =
    value: string_literal ("hello")
  ;
```
