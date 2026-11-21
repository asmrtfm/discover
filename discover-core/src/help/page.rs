//! The top-level page, `discover --help`: everything but the COMMANDS entries, which [`super::render_top`] builds from each subcommand's summary.

/// NAME, SYNOPSIS, DESCRIPTION and OPTIONS.
pub const HEAD: &str = "NAME
       discover - navigate a codebase through its import graph

SYNOPSIS
       discover [OPTION]... COMMAND [ARG]...
       discover COMMAND --help
       discover config

DESCRIPTION
       discover analyzes the codebase and caches the results, so later runs are fast. The caches rebuild on their own when files are added or removed or the entry points change. Use --rebuild-cache to force a rebuild, or --no-rebuild to use the existing caches as they are.

       Files and their importers are classified as:

       \"live\"
              Reachable from an entry point through imports. It says nothing about whether the file's code is used. Reported by live-files and is-live.

       \"dead\"
              Unreachable: no chain of importers leads back to an entry point. Reported by dead-files, which also prints a DISAGREEMENT line for any file the two checks classify differently.

       \"stale\"
              An importer that is \"live\" but uses none of the file's entities. Reported by trace.

       \"zombie\"
              An importer that is \"dead\". Reported by trace.

       trace also calls a file \"dead\" when every importer is \"stale\" or \"zombie\".

       Without --lang, the language is the first answer from: the lang key for the current directory in discover.conf (see FILES); the extension of the first argument with a known source extension; a manifest in the current directory, such as pubspec.yaml, Gemfile or package.json; the most common source file extension in the project.

OPTIONS
       Options are recognized anywhere on the command line.

       --lang=LANG
              Use the navigator for LANG (dart, java, kotlin, ruby or typescript) instead of detecting it.

       --rebuild-cache
              Delete and rebuild every cache before running COMMAND. Without a COMMAND, only rebuild.

       --no-rebuild
              Use the caches on disk even when they are stale; only missing caches are built. Cannot be combined with --rebuild-cache.

       --strict
              Skip, with a warning, any FILE that doesn't exist, instead of looking it up in the caches.

       --live-only
              Limit a command to live files.

       -v, --verbose
              Print more detail where a command supports it.

       -h, --help
              Print this page, or after a COMMAND that command's own page, and exit. With a known language, only that language's commands are listed.

       -V, --version
              Print the version and build time and exit.";

/// The line above the COMMANDS entries.
pub const COMMANDS_INTRO: &str = "       Run discover COMMAND --help for a command's own page.";

/// ENVIRONMENT and FILES, which hold for every language.
pub const TAIL: &str = "ENVIRONMENT
       DISCOVER_ENTRY=\"FILE...\"
              Entry points for this run, repo-relative and separated by whitespace, used instead of discover.conf's and the language's defaults.

       CACHE_DIR
              Where the caches are kept, instead of ./tmp.

       XDG_CONFIG_HOME
              The directory of the user's discover.conf, instead of ~/.config.

FILES
       ./discover.conf, $XDG_CONFIG_HOME/discover.conf
              A YAML (or JSON) mapping from project directories to their settings: lang, the navigator to run, and entries, the repo-relative files where the program starts, which replace the language's defaults. The first file with an entry for the current directory is used. discover config writes a commented one.

       ./tmp/
              The caches, unless CACHE_DIR is set.";
