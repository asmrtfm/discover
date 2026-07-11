# Handoff: ast-grep Dart Wrapping Heuristic (continued)

**Date**: 2026-06-02
**Repo being modified**: `/home/me/Desktop/workspace/ast-grep` (cloned from `https://github.com/ast-grep/ast-grep.git`)
**Prior handoff**: `/home/me/Desktop/workspace/malsi-app/.claude/handoffs/20260602_ast-grep-dart-fix.md` (read this first for full background on the problem)

## What changed since the prior handoff

The prior session left the code with a `direct_parse_has_errors` function that did a **full recursive subtree scan** for ERROR/MISSING nodes. That caused 3 test failures because metavariable placeholders like `$$$BODY` create ERROR nodes deep inside otherwise-valid declaration patterns.

This session replaced that with a multi-signal heuristic called `should_try_wrapping`. The function is in `/home/me/Desktop/workspace/ast-grep/crates/language/src/dart.rs`.

### The four signals in `should_try_wrapping`

1. **Root node is ERROR** — the parse totally failed (e.g. `AppIcons.$FIELD` produces ERROR as the root, not `source_file`)
2. **Direct child of source_file is ERROR** — top-level misparse (e.g. `print($A)` wraps in ERROR at the source_file level)
3. **Any MISSING node anywhere in the tree** — the grammar hallucinated a token (most commonly a missing semicolon `;`). This catches patterns like `$A = $B`, `var $A = $B`, `OverlayButton.bubble($$$)` which tree-sitter incorrectly interprets as top-level variable/function declarations with a missing semicolon
4. **Source starts with a Dart statement keyword** — catches `if`, `for`, `while`, `return`, etc. being misinterpreted as function names. The keyword list is in `DART_STATEMENT_KEYWORDS` constant at the top of the `impl Dart` block

### Candidate ordering was also swapped

The `try_wrapped_pattern` method tries two wrapping candidates:
- **Without semicolon first**: `void _() {\n<pattern>\n}` — for statement patterns (if/for/while)
- **With semicolon second**: `void _() {\n<pattern>;\n}` — for expression patterns (function calls, assignments)

The prior code had the with-semicolon candidate first. That caused the `if ($COND) { $$$BODY }` test to fail because adding `;` after `}` made tree-sitter parse `{ $$$BODY }` as a set/map literal instead of a block (Dart grammar ambiguity). The without-semicolon candidate parses it correctly as an `if_statement`.

Expression patterns like `print($A)` still work: the without-semicolon attempt fails (Dart requires `;` after expressions — the block gets ERROR children), so it falls through to the with-semicolon candidate which succeeds.

## Current test results: 13 pass, 2 fail

Run with: `cd /home/me/Desktop/workspace/ast-grep && cargo test -p ast-grep-language dart`

**Passing (13)**: class, class_with_body, replace, function_call, method_call, named_constructor, assignment, variable_declaration, member_access, return_statement, top_level_function, expression_replace, debug_parse_kinds

**Failing (2)**:

### `test_dart_import` — `import $URI`

Pattern `import $URI` at top level parses as `top_level_variable_declaration(type: import, name: $URI)` with MISSING `;`. The MISSING `;` triggers `should_try_wrapping`, but the wrapped version also fails because `import` is a keyword that cannot appear inside a function body as a statement.

This test is probably a bad test case (as noted in the prior handoff). `import` statements are top-level-only constructs in Dart. The pattern `import $URI` can never match inside a function body, and at the top level the grammar expects `import 'string_literal'` not `import $URI` (because `$URI` is treated as an identifier, not a string). The correct way to match imports in ast-grep would be using `--selector` with the `import_specification` node kind, or a pattern like `import '$URI'` with the quotes included.

**Recommendation**: Either fix the test to use `import '$URI'` (but `$` inside Dart strings is interpolation syntax, so this might also fail), or remove/skip this test and document that imports need `--selector` matching.

### `test_dart_if_statement` — `if ($COND) { $$$BODY }`

The `should_try_wrapping` function correctly identifies this needs wrapping (signal 4: starts with `if`). The wrapping logic tries the without-semicolon candidate first. **I did not get to run the tests after swapping the candidate order for this specific test** — the session was interrupted right after making that change. The swap should fix this test, but it needs verification.

If it still fails: the issue would be in `try_extract_inner`. The `{ $$$BODY }` inside the wrapped `if_statement` might produce ERROR nodes that cause the block-level check (`block.children().any(|c| c.is_error())`) to reject the extraction. Check whether `$$$BODY` (which is a valid Dart identifier due to `$` being legal) is parsed cleanly inside the if-statement's consequence block, or whether `$$$` triggers some other error.

## File state

The only file you need to edit is:
- `/home/me/Desktop/workspace/ast-grep/crates/language/src/dart.rs` — contains the `Dart` struct, `Language` + `LanguageExt` impls, wrapping logic, and all tests

Supporting changes in other files (already done, do not redo):
- `crates/core/src/lib.rs` — `mod node` changed to `pub mod node`
- `crates/core/src/matcher/pattern.rs` — added `pub fn src(&self) -> &str` on `PatternBuilder`
- `crates/language/src/lib.rs` — removed `impl_lang!(Dart, ...)` macro, added `pub use dart::Dart;`

## Debug test

There's a `test_dart_debug_parse_kinds` test in the file that dumps the parse tree for various patterns. It has an unused `dump` function that causes a compiler warning — either remove the dead `dump` function or wire it back in. The test itself is useful to keep as a diagnostic reference.

## Build/test commands

```bash
cd /home/me/Desktop/workspace/ast-grep
cargo test -p ast-grep-language dart          # run all 15 Dart tests
cargo test -p ast-grep-language dart -- --nocapture  # with stderr output (for debug test)
cargo check                                    # type-check only (~0.5s)
```

## What to do next

1. Run the tests to see if the candidate-order swap fixed the `if_statement` test
2. If not, debug the wrapped parse of `if ($COND) { $$$BODY }` — the `try_extract_inner` method might need adjustment for how it checks errors inside the block
3. Decide what to do about the `import` test (likely remove/rewrite it)
4. Clean up the unused `dump` function in `test_dart_debug_parse_kinds` (either remove it or wire it back in)
5. Run the full test suite (`cargo test`) to make sure nothing else broke — there are 80 tests across all languages
6. The ultimate goal: use ast-grep with working Dart support for a reachability tool in the malsi-app project. The trace-reachability skill lives at `/home/me/Desktop/workspace/malsi-app/.claude/skills/trace-reachability/`
