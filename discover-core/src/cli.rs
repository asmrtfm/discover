//! The command line every navigator shares: argument handling, output, and
//! the subcommands that only need the graph (live-files, dead-files,
//! is-live, importers, trace, prune). A language adds resolve, inspect,
//! definition and usages.

use crate::commands::{dead_files, importers, is_live, live_files, prune, trace};
use crate::common::out;
use crate::graph::{Graph, Lang};
use crate::help::{self, Generic, Help, Page};
// Holds the per-language source file counts during language detection.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;
use std::sync::{LazyLock, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};

/// Holds the name of the subcommand that `run` dispatches to.
static SUBCOMMAND: OnceLock<String> = OnceLock::new();

/// Whether any of `flags` is among the command-line arguments.
fn has_flag(flags: &[&str]) -> bool {
  std::env::args().skip(1).any(|arg| flags.contains(&arg.as_str()))
}

/// Whether `--strict` was given, read from the command line on first use.
pub static STRICT: LazyLock<bool> = LazyLock::new(|| has_flag(&["--strict"]));

/// Whether `-v` or `--verbose` was given, read from the command line on first use.
pub static VERBOSE: LazyLock<bool> = LazyLock::new(|| has_flag(&["-v", "--verbose"]));

/// Whether `--live-only` was given, limiting a command to live files, read from the command line on first use.
pub static LIVE_ONLY: LazyLock<bool> = LazyLock::new(|| has_flag(&["--live-only"]));

/// Holds the language named by `--lang=<lang>`, which the discover binary routes on.
pub static LANG: OnceLock<String> = OnceLock::new();

/// Holds the arguments left over for the subcommand.
static ARGS: OnceLock<Vec<String>> = OnceLock::new();

/// Holds whether `--rebuild-cache` was found in the command-line arguments. Starts false.
static REBUILD: AtomicBool = AtomicBool::new(false);

/// Holds whether `--no-rebuild` was found in the command-line arguments. Starts false.
static NO_REBUILD: AtomicBool = AtomicBool::new(false);

const COMMANDS: &[&str] = &[
  "resolve",
  "inspect",
  "live-files",
  "dead-files",
  "is-live",
  "definition",
  "importers",
  "trace",
  "prune",
  "usages",
  // Dart, TypeScript and Ruby only for now; the other languages accept it and exit 1 without output.
  "attributes",
  // Dart only for now; the other languages accept it and exit 1 without output.
  "endpoints",
];

/// The workspace version and the time of the build, written into the binary
/// at compile time by build.rs.
pub const VERSION: &str = env!("DISCOVER_VERSION");

/// Runs a navigator: `<command> [args]`, with `--rebuild-cache` or `--no-rebuild` allowed anywhere. `open` builds the
/// graph (told whether --rebuild-cache was given); `commands` handles the
/// language's own subcommands (resolve, inspect, definition, usages).
pub fn main<L: Lang>(
  open: impl FnOnce(bool) -> Result<Graph<L>, String>,
  commands: impl FnOnce(&Graph<L>, &str, &[String]) -> Result<u8, String>,
) -> ExitCode {
  match run(open, commands) {
    Ok(code) => ExitCode::from(code),
    Err(msg) => {
      eprintln!("error: {msg}");
      ExitCode::FAILURE
    }
  }
}

/// What a language exposes so that the discover binary can identify it and run it.
pub struct Language {
  /// The value of `--lang=<lang>` that selects this language.
  pub name: &'static str,
  /// The file extensions of this language's source files, without the dot.
  pub extensions: &'static [&'static str],
  /// The manifest files at a project's root that identify this language.
  pub manifests: &'static [&'static str],
  /// This language's default entry points, as the comment `discover config` generates.
  pub entry_defaults: &'static str,
  /// Runs this language's navigator.
  pub main: fn() -> ExitCode,
  /// The subcommands this language implements, which its top-level help page lists: its [`Help::COMMANDS`](crate::help::Help::COMMANDS).
  pub commands: &'static [&'static str],
  /// This language's page for a subcommand, or None when it doesn't implement it: [`help::page`](crate::help::page) for its `Help`.
  pub page: fn(&str) -> Option<Page>,
}

/// Names the directories that the source scan skips.
const SKIPPED_DIRS: &[&str] = &[".build", ".dart_tool", ".git", "build", "node_modules", "vendor"];

/// Returns the name of the language whose extensions include the path's extension, or None when no language does.
fn lang_of(langs: &[Language], path: &Path) -> Option<&'static str> {
  // Reads the path's extension.
  let ext = path.extension()?.to_str()?;
  // Finds the language that claims the extension and returns its name.
  langs.iter().find(|lang| lang.extensions.contains(&ext)).map(|lang| lang.name)
}

/// Adds one to `counts` for every source file under `dir`, recursing into every directory not in `SKIPPED_DIRS`.
fn count_langs(langs: &[Language], dir: &Path, counts: &mut BTreeMap<&'static str, usize>) {
  // Lists the directory, treating an unreadable one as empty.
  let entries = std::fs::read_dir(dir).into_iter().flatten().flatten();
  // Visits every entry in the directory.
  for entry in entries {
    // Gets the entry's full path.
    let path = entry.path();
    // Handles a real directory; symlinks aren't followed, so a link cycle can't loop forever.
    if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
      // Reads the directory's own name.
      let name = entry.file_name();
      // Recurses unless the directory is on the skip list.
      if !name.to_str().is_some_and(|name| SKIPPED_DIRS.contains(&name)) {
        // Counts the files inside the directory.
        count_langs(langs, &path, counts);
      }
    // Handles a file with a known source extension.
    } else if let Some(lang) = lang_of(langs, &path) {
      // Adds one to that language's count.
      *counts.entry(lang).or_default() += 1;
    }
  }
}

/// Returns the best guess at the language: a file argument's extension, then a manifest in the current directory, then the most common source extension in the project.
pub(crate) fn detect_lang(langs: &[Language], args: &[String]) -> Option<&'static str> {
  // Returns the language of the first argument with a known extension.
  if let Some(lang) = args.iter().find_map(|arg| lang_of(langs, Path::new(arg))) {
    // Stops at the file argument, which is the strongest signal.
    return Some(lang);
  }
  // Finds the first language with one of its manifests in the current directory.
  if let Some(lang) = langs.iter().find(|lang| lang.manifests.iter().any(|manifest| Path::new(manifest).is_file())) {
    // Stops at the manifest, which identifies the project.
    return Some(lang.name);
  }
  // Holds the number of source files found for each language.
  let mut counts = BTreeMap::new();
  // Counts the source files from the current directory down.
  count_langs(langs, Path::new("."), &mut counts);
  // Returns the language with the most files, or None when there are none.
  counts.into_iter().max_by_key(|(_, count)| *count).map(|(lang, _)| lang)
}

/// The known language's name, subcommands and page function, or core's generic ones when no language is known. The language comes from --lang= anywhere on the command line, then discover.conf, then detection, like parse_args.
fn help_source(langs: &[Language], args: &[String]) -> (Option<&'static str>, &'static [&'static str], fn(&str) -> Option<Page>) {
  // Takes the language from --lang=, then discover.conf, then detection.
  let name = args
    .iter()
    .find_map(|arg| arg.strip_prefix("--lang=").map(str::to_string))
    .or_else(|| crate::project::project_config().and_then(|config| config.lang()))
    .or_else(|| detect_lang(langs, args).map(str::to_string));
  match name.as_deref().and_then(|name| langs.iter().find(|lang| lang.name == name)) {
    // Uses the language's own pages.
    Some(lang) => (Some(lang.name), lang.commands, lang.page),
    // Uses core's generic pages for every public subcommand.
    None => (None, Generic::COMMANDS, help::page::<Generic>),
  }
}

/// The top-level page for these subcommands, each listed with the summary from its page.
fn top_page(commands: &[&str], page: fn(&str) -> Option<Page>) -> String {
  help::render_top(&commands.iter().filter_map(|command| page(command).map(|p| (*command, p))).collect::<Vec<_>>())
}

/// `-h` / `--help`: prints the page of the subcommand on the command line, or the top-level page when there is none, to stdout and exits 0, without opening a project.
fn help(langs: &[Language]) -> ! {
  // Reads the whole command line, since --lang= and the subcommand may come after --help.
  let args: Vec<String> = std::env::args().skip(1).collect();
  let (lang, commands, page) = help_source(langs, &args);
  // Builds the page of the first word that names a subcommand, or the top-level page.
  let text = match args.iter().find(|arg| *arg == "config" || COMMANDS.contains(&arg.as_str())) {
    Some(command) => match page(command) {
      Some(p) => help::render(command, &p),
      // Refuses a subcommand the language doesn't implement.
      None => {
        eprintln!("error: {} has no {command} command", lang.unwrap_or("discover"));
        std::process::exit(1)
      }
    },
    None => top_page(commands, page),
  };
  // Prints the page filled to the line width, as man would.
  out(&help::fill(&text));
  // Exits without running anything else.
  std::process::exit(0)
}

/// Sets the command-line globals from the arguments. The discover binary calls this once, before anything else runs.
pub fn parse_args(langs: &[Language]) {
  // Collects the arguments that belong to the subcommand.
  let mut args: Vec<String> = Vec::new();
  // Visits every argument after the binary's name.
  for arg in std::env::args().skip(1) {
    // Once set, a word matching a subcommand name is an argument, so `usages trace` searches for `trace`. Global flags are recognized in any position.
    let no_subcommand_yet = SUBCOMMAND.get().is_none();
    // Sorts the argument into a global or the subcommand's arguments.
    match arg.as_str() {
      // Keeps the flags that STRICT, VERBOSE and LIVE_ONLY read away from the subcommand.
      "--strict" | "-v" | "--verbose" | "--live-only" => {}
      // Prints the help page and exits; no project or caches involved.
      "-h" | "--help" => help(langs),
      // Records the language, keeping the first one given.
      flag if flag.starts_with("--lang=") => {
        // Stores the text after `--lang=`; a later --lang is ignored.
        let _ = LANG.set(flag.trim_start_matches("--lang=").to_string());
      }
      // --version: the build's baked-in version; no project or caches involved.
      "--version" | "-V" => {
        // Prints the binary's name and version.
        out(&format!("discover {VERSION}"));
        // Exits without running anything else.
        std::process::exit(0);
      }
      // Records that the caches should be rebuilt.
      "--rebuild-cache" => REBUILD.store(true, Ordering::Relaxed),
      // Records that the caches on disk should be used as they are.
      "--no-rebuild" => NO_REBUILD.store(true, Ordering::Relaxed),
      // Records the first word that names a subcommand.
      name if no_subcommand_yet && COMMANDS.contains(&name) => {
        // Stores the subcommand, which can only be unset here because of the guard.
        SUBCOMMAND.set(arg.clone()).expect("set once, guarded by no_subcommand_yet");
      }
      // Passes everything else to the subcommand.
      _ => args.push(arg),
    }
  }
  // Detects the language when --lang wasn't given.
  if LANG.get().is_none() {
    // Tries the config file first, then detection, keeping whichever finds a language.
    if let Some(lang) = crate::project::project_config().and_then(|config| config.lang()).or_else(|| detect_lang(langs, &args).map(str::to_string)) {
      // Sets LANG, which is known to be empty here.
      let _ = LANG.set(lang);
    }
  }
  // Prints the top-level page to stderr and fails when there is nothing to run: no subcommand and no --rebuild-cache.
  if SUBCOMMAND.get().is_none() && !REBUILD.load(Ordering::Relaxed) {
    let (_, commands, page) = help_source(langs, &std::env::args().skip(1).collect::<Vec<_>>());
    eprintln!("{}", help::fill(&top_page(commands, page)));
    std::process::exit(1);
  }
  // Stores the subcommand's arguments.
  ARGS.set(args).expect("parse_args runs once");
}

fn run<L: Lang>(
  open: impl FnOnce(bool) -> Result<Graph<L>, String>,
  commands: impl FnOnce(&Graph<L>, &str, &[String]) -> Result<u8, String>,
) -> Result<u8, String> {
  // Reads whether --rebuild-cache was given.
  let rebuild = REBUILD.load(Ordering::Relaxed);
  // Reads whether --no-rebuild was given.
  let no_rebuild = NO_REBUILD.load(Ordering::Relaxed);
  // Reads the subcommand's arguments.
  let args = ARGS.get().map_or(&[][..], Vec::as_slice);
  if rebuild && no_rebuild {
    return Err("--rebuild-cache and --no-rebuild cannot be combined".into());
  }
  // --no-rebuild: use the caches on disk as they are (see cache::skip_rebuild).
  if no_rebuild {
    crate::cache::skip_rebuild();
  }
  // Empty only with --rebuild-cache, since parse_args prints the top-level page when there is nothing to run.
  let command = SUBCOMMAND.get().map_or("", String::as_str);

  let mut graph = open(rebuild)?;
  if rebuild {
    graph.build_all()?;
    // With no subcommand, the rebuild was the whole request.
    if command.is_empty() {
      return Ok(0);
    }
  }

  match command {
    // A core subcommand the language replaces goes to the language.
    name if L::OVERRIDES.contains(&name) => commands(&graph, name, &args),
    "live-files" => live_files::cmd_live_files(&graph),
    "dead-files" => dead_files::cmd_dead_files(&graph),
    "is-live" => is_live::cmd_is_live(&graph, &args),
    "importers" => importers::cmd_importers(&graph, &args),
    "trace" => trace::cmd_trace(&graph, &args),
    "prune" => prune::cmd_prune(&mut graph, &args),
    other => commands(&graph, other, &args),
  }
}
