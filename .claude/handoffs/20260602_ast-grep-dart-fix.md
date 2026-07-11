# Handoff: Fixing ast-grep Dart Language Support

**Date**: 2026-06-02
**Repo being modified**: `/home/me/Desktop/workspace/ast-grep` (cloned from `https://github.com/ast-grep/ast-grep.git`)
**Motivation**: The malsi-app project has a `trace-reachability` skill at `.claude/skills/trace-reachability/` that determines whether Dart symbols are actually used by the app. The user wanted to replace the regex-based approach with ast-grep for structural matching, but discovered ast-grep 0.42.3's Dart support is broken — no expression-level patterns work at all. The user told me to clone the repo and fix it.

## What was done in this session

### 1. Audited the trace-reachability skill
- Reviewed `SKILL.md` and `scripts/trace-asset-reachability.sh`
- Concluded: the shell script is asset/icon-specific; the SKILL.md procedure is general-purpose
- Also reviewed `malsi/latest/scripts/find_unreachable_dart.dart` — it's a Dart reimplementation of the same BFS import-graph walk, NOT using the Dart analyzer SDK as initially assumed
- Copied the shell script into the skill directory: `.claude/skills/trace-reachability/scripts/trace-asset-reachability.sh`

### 2. Discovered ast-grep Dart is broken
- ast-grep 0.42.3 is installed on the system and works fine for JavaScript
- For Dart, every pattern — even `print("hello")` or a bare identifier — fails to match anything
- Root cause: Dart's tree-sitter grammar only accepts **declarations** at the top level of a source file. Expression-level patterns like `print($A)` get misinterpreted as function signatures, variable declarations, or type references

### 3. Cloned ast-grep and implemented a fix
The fix is in 4 files across 2 crates:

#### `crates/core/src/lib.rs`
- Changed `mod node;` to `pub mod node;` so the `Root` type is accessible from the language crate

#### `crates/core/src/matcher/pattern.rs`
- Added `pub fn src(&self) -> &str` getter on `PatternBuilder` so custom `build_pattern` implementations can access the pattern source text

#### `crates/language/src/lib.rs`
- Removed `impl_lang!(Dart, language_dart);` (the stub macro that generated a minimal Dart struct)
- Added `pub use dart::Dart;` to re-export the custom implementation
- Added a comment explaining the custom impl lives in dart.rs

#### `crates/language/src/dart.rs`
- **Was**: a `#![cfg(test)]`-only file with 3 basic tests (class matching only)
- **Now**: a full custom `Language` + `LanguageExt` implementation with the wrapping strategy

**The wrapping strategy**:
1. Parse the pattern source as-is (top-level Dart)
2. Check the parse tree for ERROR or MISSING nodes via `direct_parse_has_errors()` (recursive full-tree scan)
3. If errors found, wrap the pattern in `void _() {\n<pattern>;\n}` and try to extract the inner expression/statement node from inside the `block`
4. Two wrapping attempts: with semicolon first (for expressions), then without (for statements like `if`/`for`)
5. `try_extract_inner()` finds the `block` node, rejects if block contains ERROR nodes, extracts the first named child
6. If the extracted node is an `expression_statement` (from the added semicolon), unwrap one level deeper to get the raw expression (so `print($A)` matches `call_expression`, not `expression_statement`)
7. If wrapping fails or produces errors, fall back to the direct parse

## Current test results: 11 pass, 3 fail

**Passing** (11): class, class_with_body, replace, function_call, method_call, named_constructor, assignment, variable_declaration, member_access, return_statement, expression_replace

**Failing** (3):

### `test_dart_import` — `import $URI`
- `import` is a keyword that the tree-sitter grammar interprets as a type name when used inside a function body
- At top level, `import $URI` becomes `top_level_variable_declaration(type: import, name: $URI)` with a MISSING semicolon
- The MISSING node triggers the fallback, but the wrapped version also misparses (`import` becomes a type name inside the function body)
- **This test is probably wrong** — `import $URI` can't work because `$URI` would need to be a string literal. The correct pattern would need quotes, but `$` inside Dart strings is interpolation syntax. This may need a different approach (like matching the `import_specification` node kind directly via `--selector`)

### `test_dart_top_level_function` — `void $NAME($$$) { $$$BODY }`
- The pattern contains `$$$BODY` inside `{ }`, which tree-sitter can't parse (it's not valid Dart), creating an ERROR node inside the function body
- `direct_parse_has_errors` returns `true` (because of the `$$$BODY` ERROR node deep in the tree)
- The fallback wrapping produces a `local_function_declaration` (different node kind from top-level `function_declaration`), so it doesn't match
- **Root cause**: `subtree_has_errors` is too aggressive — it catches ERROR nodes from metavariable placeholders deep in the tree that the existing matcher already handles fine

### `test_dart_if_statement` — `if ($COND) { $$$BODY }`
- Same root cause as above — `$$$BODY` inside `{ }` creates parse errors
- Additionally, tree-sitter-dart has an ambiguity: `{ $$$BODY }` after `if (cond)` can be parsed as either a `block` or a `set_or_map_literal`, and the grammar sometimes picks the wrong one

## The key unsolved problem

The error detection heuristic (`direct_parse_has_errors`) needs to distinguish between:
1. **"The grammar misinterpreted this pattern as the wrong kind of declaration"** — e.g. `print($A)` becoming a function signature → must use wrapped version
2. **"The pattern has metavariable artifacts that create ERROR/MISSING nodes but the overall structure is correct"** — e.g. `void $NAME($$$) { $$$BODY }` is a valid function declaration with `$$$` creating benign errors → must use direct version

**The approach I was about to try when interrupted**: check only ERROR nodes that are **direct children of `source_file`** (not the full tree). Metavariable errors from `$$$` are always deeper in the tree (inside parameter lists, class bodies, function bodies). Top-level misinterpretation errors (like `print($A)` being wrapped in ERROR at the source_file level) ARE direct children. This would correctly distinguish the two cases.

The tricky case is `AppIcons.$FIELD` which at top level parses as `type(AppIcons.type_identifier($FIELD))` — no ERROR at the source_file level. This one parses "cleanly" as a type reference. To catch this, one option is to also check if the source_file's child is a `type` node (which is never a valid standalone top-level declaration in Dart — types are always part of larger declarations). A list of "this is not a real top-level declaration" node kinds could be maintained.

Actually, re-examining the CST output: `AppIcons.$FIELD` at top level gives:
```
source_file > ERROR > type > type_identifier . type_identifier
```
The ERROR IS a direct child of source_file. So checking only direct children of source_file for ERROR should work for this case too. The test I ran before that showed `type` as the direct child may have been a different version of the code. **Verify this by re-running with the "direct children only" heuristic.**

## Files to clean up

Test scratch files in `.claude/skills/trace-reachability/scripts/`:
- `ast_test.dart`, `ast_test2.dart`, `ast_test3.dart`, `ast_test.js`, `ast_test.go`
These were created during ast-grep exploration. They can be deleted.

## What the user ultimately wants

1. Fix ast-grep Dart support so expression/statement patterns work
2. Build a new reachability script using ast-grep instead of regex for structural matching
3. The script should produce identical output to the existing `trace-asset-reachability.sh` for comparison
4. Eventually generalize the tool beyond just assets to arbitrary Dart symbols

## Relevant context

- The user has ast-grep installed at the system level (version 0.42.3)
- The ast-grep repo is at `/home/me/Desktop/workspace/ast-grep` — NOT inside the malsi-app org directory
- The malsi Flutter app is at `malsi/latest/` (symlink to `malsi/20260523/212956/malsi/`)
- Rust toolchain is available for building ast-grep
- Build command: `cd /home/me/Desktop/workspace/ast-grep && cargo check` (full build takes ~6s)
- Test command: `cd /home/me/Desktop/workspace/ast-grep && cargo test -p ast-grep-language dart`
- The full test suite has 80 tests across all languages; only the 14 Dart tests need attention
