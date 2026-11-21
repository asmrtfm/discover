# discover: layout rules

The layout is a fractal: every language crate mirrors the layout of `discover-core`. Code goes in the file its purpose names, never in whichever file is open.

## The tree

```
discover-core/src/
├── lib.rs              # module declarations only
├── cli.rs              # argument parsing, language selection, run, dispatch
├── common.rs           # logic shared by more than one subcommand
├── graph.rs, project.rs, cache.rs, memo.rs, par.rs   # infrastructure
├── commands/           # one file per subcommand (purpose, arguments, output shape)
│   ├── resolve.rs, inspect.rs, definition.rs, usages.rs, attributes.rs, endpoints.rs
│   └── live_files.rs, dead_files.rs, is_live.rs, importers.rs, trace.rs, prune.rs, config.rs
└── help/
    ├── mod.rs          # the `Help` trait, page assembly, fill
    ├── page.rs         # the top-level page: NAME, SYNOPSIS, DESCRIPTION, OPTIONS, COMMANDS summaries, ENVIRONMENT, FILES
    └── <subcommand>.rs # one per subcommand: its generic page and its summary

discover-<lang>/src/
├── lib.rs              # module declarations, LANGUAGE, dispatch
├── <lang>.rs           # parsing and the per-file index only
├── graph.rs, project.rs
├── common.rs           # logic shared by more than one of this language's subcommands
├── commands/<subcommand>.rs   # one per subcommand the language supports
└── help/
    ├── mod.rs          # `impl Help`: supported subcommands, plus which entries are overridden
    └── <subcommand>.rs # only where this language has something specific to say on that subcommand's page
```

`discover-jvm` follows the language layout for everything Java and Kotlin share; `discover-java` and `discover-kotlin` hold only what differs.

## Rules

- Every subcommand has a dedicated file under `commands/`. Nothing else goes in that file, and that subcommand's code goes nowhere else.
- A subcommand file never imports another subcommand's file. Logic used by more than one subcommand goes in that crate's `common.rs`.
- `<lang>.rs` holds parsing and the per-file index (what `graph.rs` and the caches are built from). Code only one subcommand uses does not go there, even if it walks the parse tree.
- `lib.rs` holds module declarations, the `LANGUAGE` value and the dispatch table. No subcommand logic, no help text.
- A subcommand's purpose and output are the same in every language; only its implementation differs. A language that can't support a subcommand doesn't implement it, and the subcommand is left off its help page.
- A language that replaces a core subcommand (Ruby's `trace`) does it in its own `commands/<subcommand>.rs`, like any other subcommand.

## Help

- Help works like `gh`: `discover --help` prints the top-level page, whose COMMANDS section lists each subcommand with a short summary, and every subcommand has its own page (`discover <subcommand> --help`) with its own NAME, SYNOPSIS, DESCRIPTION, OPTIONS and so on.
- All help text and all logic that prints it live under `help/`, never in `cli.rs` or `lib.rs`.
- `discover-core/src/help/page.rs` holds the top-level page. `discover-core/src/help/<subcommand>.rs` holds each subcommand's generic page and summary. The `Help` trait has one method per subcommand, defaulting to that page.
- A language's help calls the core page for every subcommand it supports. Only when its implementation has something specific that must be mentioned does it add `help/<subcommand>.rs` with a function of the same name, and override that one trait method with it, replacing only the parts of the page that differ.
- Every page, top-level or subcommand, follows the man page format: section headings at the margin, text indented 7 columns, entry descriptions 14. Each paragraph is one line in the source; `fill` breaks it to the line width when printed.

## Working in this repo

- Explain every change before making it.
- One edit or command at a time; no batching, no parallel calls.
