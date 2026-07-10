# Handoff: Language-Specific Usage Guides for discover

## Goal

When a project onboards ast-grep via `discover init <lang>`, the installation currently produces a `SKILL.md` that tells Claude *how to call* discover's subcommands. What's missing is a companion document that teaches the user (or Claude) *how to use ast-grep patterns for that language* — what questions can be asked, what the pattern syntax means, and what real patterns look like.

There is no generic one-size-fits-all here. Dart's ast-grep patterns look nothing like Swift's or Ruby's. The AST node kinds are different, the import models are different, the entity types are different. Each language needs its own guide written from the real information in its lang config and entity extraction implementation.

## What gets created

One file per language: `langs/<lang>-usage.md`

Six languages: dart, swift, ruby, typescript, crystal, bash.

Each file covers exactly these five things:

1. **What structural questions this language's discover can answer** — not generic "find definitions" but the language-specific version: what's unique about this language's linking/module system that makes discover necessary here.

2. **ast-grep pattern syntax with language-specific examples** — what `$VAR`, `$$$ARGS`, `--kind` mean in the context of this language's tree-sitter grammar. Some languages use `impl_lang!` (Bash — `$` is native), others use `impl_lang_expando!` (Swift, Ruby, Crystal, TypeScript — `$` is replaced by an expando char internally, but the user still writes `$VAR`). Dart and Crystal have custom `Language` impls. The user doesn't need to know the Rust internals, but they do need to know what patterns are valid.

3. **Real patterns for common questions people ask about codebases in that language** — not toy examples, but the patterns someone would actually reach for. These come from the `SKILL_USAGE_EXAMPLE_*` values in the lang configs and from what the entity extraction functions actually search for.

4. **The language's AST node kinds** — the ones our `ast_grep_entities` function actually uses, and what source constructs they match. This is language-specific because tree-sitter grammars name things differently (Ruby has `method` and `singleton_method`, Swift uses `class_declaration` for structs/enums/classes/actors/extensions, Dart has separate `class_declaration`/`enum_declaration`/`mixin_declaration`/etc).

5. **Import/linking model explanation** — how this language connects files together and how discover resolves those connections. Dart has 3 directives and 2 URI schemes. Swift has module-level imports where all files in a target are peers. Ruby has `require` vs `require_relative`. Crystal has glob requires. TypeScript has extensionless resolution and barrel re-exports. Bash has `source` and `.` (dot).

## Where the source information lives

All of this information already exists in the codebase:

- `langs/<lang>.sh` — the lang config has the import resolution logic, AST node kinds, entity extraction, and skill metadata variables
- `crates/language/src/lib.rs` — which macro each language uses (`impl_lang!` vs `impl_lang_expando!` vs custom impl)
- `crates/language/src/dart.rs` and `crystal.rs` — custom wrapping strategies for languages whose grammars reject expression-level patterns at top level
- `lib/core.sh` — the shared engine that the lang configs plug into
- `tests/fixtures/<lang>.<ext>` — representative source files with real code

## How they get installed

Task #14: modify `lib/init.sh` to copy `langs/<lang>-usage.md` into the project's `.claude/skills/discover/USAGE.md` alongside the existing `SKILL.md`. Simple file copy, no templating.

## Process

One language at a time, sequentially. User reviews each before moving to the next. No batching, no parallelization.

## Task state

Exported to `.claude/tasks.json`. Import with `/tasks import`.
