Fix slop'd --help info
write real tests
trace help: "a FILE that isn't a source file is skipped" is misleading; it means a file in a language trace doesn't parse (TS: non-scripts like .css; Ruby: non-.rb unless an entry). Reword in core, list per-language types in each language's help/trace.rs


discover-dart/src/commands/endpoints.rs
"Only live files are walked unless `all` is set." <-- fix this (it should behave like the other commands where `--live-only` (or whatever the flag is) dictates whether all or limited to live files and all is default)
