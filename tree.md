.
├── Cargo.lock               # Pinned versions of every dependency in the workspace.
├── Cargo.toml               # Workspace: members `discover` and `discover-*`; release profile with LTO; shared version, edition and license; workspace deps ast-grep-core and ast-grep-language (from the asmrtfm/ast-grep fork), ignore and serde.
│
├── discover
│   ├── Cargo.toml           # The `discover` binary; depends on discover-core and every language crate (dart, java, kotlin, ruby, typescript).
│   └── src
│       └── main.rs          # `LANGS`, the `Language` value of each language crate, and `main()`, which hands them to discover-core's CLI to pick a language and run it.
│
├── discover-core
│   ├── build.rs             # Writes the workspace version and the UTC build time into `DISCOVER_VERSION`, which `--version` prints.
│   ├── Cargo.toml           # The shared engine crate; depends on serde, serde_json and serde_yaml.
│   ├── src
│   │   ├── cache.rs         # On-disk caches: `Caches` and `Slot` (one cache file plus its `.fp` fingerprint stamp), `skip_rebuild()` for --no-rebuild, line-file read/write helpers, and the `Fnv` hash used for fingerprints.
│   │   ├── cli.rs           # Global flag parsing (`parse_args`), language detection, the `Language` struct, `run()`/`main()`, stdout helpers (`out`, `out_json`), argument parsers for resolve/inspect/definition/usages, the definition/entity/usages output shapes, `discover config`, the shared commands is-live/importers/prune/trace, and the whole help page (text, `help()`, `fill()`).
│   │   ├── graph.rs         # The `Lang` trait each language implements; `Graph<L>` with its cached views (live files, the importers index of `Row`/`EntityUse`, dead files); `Trace` for walking importers back to an entry point.
│   │   ├── lib.rs           # Declares the modules: cache, cli, graph, memo, par, project, usages.
│   │   ├── memo.rs          # `Memo<K, V>`: a mutex-guarded map that computes each key's value once (parsed facts per file, patterns per name).
│   │   ├── par.rs           # `par_map()`: an order-preserving map over a slice, run on as many threads as there are cores.
│   │   ├── project.rs       # Project plumbing: current dir, manifest check, cache dir, path conversions, git listing, directory walk, discover.conf (`ProjectConfig`, `project_config()`) and custom entry points.
│   │   └── usages.rs        # The `Usage` row every language's `usages` prints, with `line_text()`, `sorted_unique()` and `file_line()`.
│   │
│   └── tests                # Probably bogus "tests".
│
│
├── discover-dart
│   ├── Cargo.toml           # The Dart crate; depends on ast-grep-core, ast-grep-language, discover-core, ignore, regex, serde and serde_json.
│   ├── src
│   │   ├── attributes.rs    # The `attributes` command: `cmd_attributes()`, plus `Attribute` and `attributes()`, which read a class's instance fields (name and type) off the parse tree.
│   │   ├── common.rs        # `project_facts()`: every source file and its generated `.g.dart`/`.freezed.dart` files, each with its parsed `Facts`; used by `definition` and `usages`.
│   │   ├── dart.rs          # Dart parsing and the per-file index: `parse()`, `Directive`, `Entity`, `Definition`, `Facts` and `extract()` (imports, exports, parts, top-level entities, and every named declaration including methods, accessors and constructors), plus `references()` and the helpers they use.
│   │   ├── definition.rs    # The `definition` command: `cmd_definition()`, `all_definitions()` (no FILE: every match in the project, constructors left out) and `search_definition()` (FILE's own definitions, then its imports and re-exports, then its parts).
│   │   ├── endpoints.rs     # The `endpoints` command's engine: regexes for Retrofit annotations, baseUrl, API roots and absolute URLs; `url_sites()`; `Scope`/`scopes()` (which entity or member each line belongs to); the `Walker` that climbs the call stack through the importers index; and `endpoints()`.
│   │   ├── graph.rs         # `DartLang` (implements discover-core's `Lang`): memoized facts and patterns, entry points (`ENTRY` = lib/main.dart or custom ones), live-file reachability over import/export/part, the importers index with the entities each importer uses, and the cache fingerprint.
│   │   ├── lib.rs           # The Dart `USAGE` help text, the `LANGUAGE` value, `main()` with the command dispatch, and the commands still inline: `cmd_resolve`, `cmd_inspect` (with its output structs), `cmd_usages` and `cmd_endpoints`.
│   │   ├── project.rs       # `Project`: root, lib/, cache dir, package name from pubspec.yaml, the files under analysis, and URI-to-file resolution (`package:` and relative URIs); generated-file helpers `is_generated()` and `generated_extensions()`.
│   │   └── usages.rs        # The `usages` engine: `Hit`/`find_all()`, the search over lib/ with an ast-grep pattern, `drop_definitions()` (leaves out the symbol's own declaration lines unless --verbose), `pattern_usages()`, and `asset_usages()` (`AssetRefs`) for --assets.
│   │
│   └── tests                # Probably bogus "tests".
│
│
├── discover-java
│   ├── Cargo.toml           # The Java crate; depends on ast-grep-core, ast-grep-language, discover-core and discover-jvm.
│   ├── src
│   │   ├── java.rs          # `Java`, discover-jvm's `Jvm` implementation for Java: `parse_import()`, `extract()` (package, imports, top-level types, whether the file has a `main`, referenced names) and `literal_lines()`.
│   │   └── lib.rs           # The `LANGUAGE` value (name `java`, extension `.java`, no manifests since build.gradle and pom.xml fit Kotlin too, default entry points, help taken from discover-jvm's `USAGE`) and `main()`, which runs discover-jvm with `Java`.
│   │
│   └── tests                # Probably bogus "tests".
│
│
├── discover-jvm
│   ├── Cargo.toml           # The code Java and Kotlin share; depends on ast-grep-core, ast-grep-language, discover-core, serde and serde_json.
│   ├── src
│   │   ├── cli.rs           # The JVM `USAGE` help text, `main()` with the command dispatch, and the commands `cmd_resolve`, `cmd_inspect`, `cmd_definition` and `cmd_usages`, plus `OrderedObject` (JSON keys in a fixed order).
│   │   ├── graph.rs         # `JvmLang<J>` (implements discover-core's `Lang`): `open()`, building and caching the declaration index, `decls()`, entry points, and the dependency edges that live files and the importers index are built from.
│   │   ├── index.rs         # The declaration index: `FileIndex` (a file's package and top-level declarations), `Index`, `Hit`, `Edge` and `Tables`, which resolve a name through imports, wildcard imports and the file's own package.
│   │   ├── lib.rs           # The `Jvm` trait each language implements (name, cache prefix, grammar, extension, git pathspec, `extract()`, `literal_lines()`), `parse()`, and the shared types `Root`, `Idents`, `Import`, `Decl` and `Facts`.
│   │   ├── project.rs       # `Project<J>`: root, source files (tests and buildSrc excluded) and entry points (`main` functions, Android manifest components, META-INF/services providers); XML helpers `strip_xml_comments()`, `tags()`, `tags_named()`, `attribute()`, and `is_test_or_build_logic()`.
│   │   └── usages.rs        # `pattern_usages()` (ast-grep search over the sources, optionally live files only) and `asset_usages()`, whose `AssetRefs` holds Android resource constants (`R.<type>.<name>`), XML references (`@<type>/<name>`), and string literals naming the file (by its path inside assets/ or resources/, else repo-relative).
│   │
│   └── tests                # Probably bogus "tests".
│
│
├── discover-kotlin
│   ├── Cargo.toml           # The Kotlin crate; depends on ast-grep-core, ast-grep-language, discover-core and discover-jvm.
│   ├── src
│   │   ├── kt.rs            # `Kotlin`, discover-jvm's `Jvm` implementation for Kotlin: `parse_import()`, `extract()` (package, imports, top-level declarations, whether the file has a `main`, top-level or `@JvmStatic` inside an object or companion object, referenced names) and `literal_lines()`.
│   │   └── lib.rs           # The `LANGUAGE` value (name `kotlin`, extension `.kt`, no manifests since build.gradle and pom.xml fit Java too, default entry points, help taken from discover-jvm's `USAGE`) and `main()`, which runs discover-jvm with `Kotlin`.
│   │
│   └── tests                # Probably bogus "tests".
│
│
├── discover-ruby
│   ├── Cargo.toml           # The Ruby crate; depends on ast-grep-core, ast-grep-language, discover-core, serde and serde_json.
│   └── src
│       ├── attributes.rs    # The `attributes` command (`run()`): for an ActiveRecord model, its table's columns from db/schema.rb, `attribute` declarations and associations (plus `attr_*` names with --verbose); for any other class, its `attr_*` names and `attribute` declarations. Includes the superclass-chain walk, column-to-Ruby type mapping, and the table-name inflection (`underscore`, `pluralize`, `singularize` with irregular and uncountable words).
│       ├── graph.rs         # `RbLang` (implements discover-core's `Lang`): each file's `Link`s (resolved require-style specs, superclasses and mixins, and under Zeitwerk the constants it references), live-file reachability over them, the importers index, and the cache fingerprint.
│       ├── lib.rs           # The Ruby `USAGE` help text, the `LANGUAGE` value, `main()` with the command dispatch (routing `trace` to trace.rs), and the commands `cmd_resolve` (paths and constants), `cmd_inspect` (with its output structs), `cmd_definition`, `all_definitions` and `cmd_usages`.
│       ├── project.rs       # `Project`: root, source files (`.rb` and `.rake`, tests and vendored code excluded), Zeitwerk autoload roots (Rails apps and gems using `Zeitwerk::Loader.for_gem`), path and constant resolution, and entry points (bin/ and exe/ executables, the gemspec's lib file, Rakefile, config.ru, rake tasks, what Rails loads by convention, else main.rb or app.rb).
│       ├── rb.rs            # Ruby parsing and the per-file index: `parse()`, ERB-to-Ruby extraction (`erb_code`), `Directive`, `Spec`, `ConstRef`, `Ancestor`, `Entity`, `Facts` and `extract()`; `literal_lines()`; `ClassBody`/`class_body()` and `Association` for `attributes`; `schema_columns()` from db/schema.rb; constant naming (`camelize`, `constant_for`, `acronyms`); and `RailsConfig`/`rails_config()` (from config/application.rb: whether it calls `autoload_lib`, the lib subdirectories it ignores, and extra `autoload_paths`/`eager_load_paths`).
│       ├── trace.rs         # Ruby's own `trace`, which replaces the shared one: `TraceArgs`/`trace_args()` (--include-upstream, --verbose), finding where a file's entities are used through each importer (in the importer, in classes and modules downstream of a module, and at a class's callsites including ERB), and the JSON output with one `entities_used` entry per line.
│       └── usages.rs        # `pattern_usages()` (ast-grep search over the sources, optionally live files only) and `asset_usages()`/`asset_literals()` (string literals in live Ruby and ERB files naming the asset by repo path, public/ URL, or app/assets logical path).
│
│
└── discover-typescript
    ├── Cargo.toml           # The TypeScript/JavaScript crate; depends on ast-grep-core, ast-grep-language, discover-core, serde and serde_json.
    ├── src
    │   ├── graph.rs         # `TsLang` (implements discover-core's `Lang`): memoized facts, live-file reachability over import, export and dynamic-import specifiers, the importers index with the entities each importer uses (`used_entities()`), and the cache fingerprint.
    │   ├── lib.rs           # The TypeScript `USAGE` help text, the `LANGUAGE` value (extensions .ts .tsx .mts .cts .js .jsx .mjs .cjs, manifest package.json), `main()` with the command dispatch (`endpoints` exits 1 with no output), and the commands `cmd_resolve`, `cmd_inspect` (with its output structs), `cmd_definition`/`search_definition`, `cmd_usages` and `cmd_attributes`.
    │   ├── project.rs       # `Project`: root, source files (declaration files and tests excluded), tsconfig path aliases and baseUrl (following `extends`, read as JSONC), module resolution, and entry points (package.json source/module/main/bin, node commands in package.json scripts, Dockerfile CMD/ENTRYPOINT and docker-compose command, else index/server/app/main at the root or in src/).
    │   ├── ts.rs            # TypeScript/TSX parsing and the per-file index: grammar choice per file (`lang_for`), `parse()`, `Directive`, `Spec`, `Entity`, `Binding` (which local name comes from which specifier), `Facts` and `extract()`; `name_uses()` and `literal_lines()`; and `Attribute`/`attributes()` for class fields and Mongoose schema fields.
    │   └── usages.rs        # `pattern_usages()` (ast-grep search over the sources, each file with its own grammar, optionally live files only) and `asset_usages()`, whose `AssetRefs` holds uses through a name bound to the asset (`import logo from …`, `const logo = require(…)`) and direct references (unbound import()/require(), or a string literal of its path).
    │
    └── tests                # Probably bogus "tests".
