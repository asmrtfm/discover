//! The command line every navigator shares: argument handling, output, and
//! the subcommands that only need the graph (live-files, dead-files,
//! is-live, importers, trace, prune). A language adds resolve, inspect,
//! definition and usages.

use crate::graph::{EntityUse, Graph, Lang, Row, Trace};
use crate::project::{relative, to_abs};
use crate::usages::Usage;
use serde::Serialize;
// Holds the per-language source file counts during language detection.
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Holds the name of the subcommand that `run` dispatches to.
static SUBCOMMAND: OnceLock<String> = OnceLock::new();

/// Holds whether `run` found `--strict` in the command-line arguments. Starts false.
static STRICT: AtomicBool = AtomicBool::new(false);

/// Holds whether `-v` or `--verbose` was found in the command-line arguments. Starts false.
pub static VERBOSE: AtomicBool = AtomicBool::new(false);

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

/// Writes a line to stdout; a closed pipe (`| head`) ends the run quietly.
pub fn out(line: &str) {
  let mut stdout = io::stdout().lock();
  if writeln!(stdout, "{line}").is_err() {
    std::process::exit(0);
  }
}

/// Pretty JSON, indented like jq's default output.
pub fn out_json<T: Serialize>(value: &T) {
  out(&serde_json::to_string_pretty(value).expect("serializable"));
}

/// Runs a navigator: `<command> [args]`, with `--rebuild-cache` or `--no-rebuild` allowed anywhere. `open` builds the
/// graph (told whether --rebuild-cache was given); `commands` handles the
/// language's own subcommands (resolve, inspect, definition, usages).
pub fn main<L: Lang>(
  usage: &str,
  open: impl FnOnce(bool) -> Result<Graph<L>, String>,
  commands: impl FnOnce(&Graph<L>, &str, &[String]) -> Result<u8, String>,
) -> ExitCode {
  match run(usage, open, commands) {
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
  /// The help text for this language's own commands, which continues the shared COMMANDS section: entries indented 7 columns, their descriptions 14, each paragraph on one line.
  pub usage: &'static str,
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
fn detect_lang(langs: &[Language], args: &[String]) -> Option<&'static str> {
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

/// `-h` / `--help`: prints the help page to stdout and exits 0, without opening a project. With a known language (`--lang=` anywhere on the command line, discover.conf, or detection), the page ends with that language's commands; otherwise it ends with every language's commands, each under its name.
fn help(langs: &[Language]) -> ! {
  // Reads the whole command line, since --lang= may come after --help.
  let args: Vec<String> = std::env::args().skip(1).collect();
  // Takes the language from --lang=, then discover.conf, then detection, like parse_args.
  let name = args
    .iter()
    .find_map(|arg| arg.strip_prefix("--lang=").map(str::to_string))
    .or_else(|| crate::project::project_config().and_then(|config| config.lang()))
    .or_else(|| detect_lang(langs, &args).map(str::to_string));
  // Builds the page for the known language, or for all of them.
  let text = match name.as_deref().and_then(|name| langs.iter().find(|lang| lang.name == name)) {
    // Continues the shared commands with this language's, then the footer.
    Some(lang) => usage_page(&format!("discover --lang={}", lang.name), lang.usage),
    // Follows the shared commands with a section of each language's, headed by its name and the flag that selects it, then the footer.
    None => {
      let mut text = shared_usage("discover");
      for lang in langs {
        text.push_str(&format!("\n\n{} COMMANDS (--lang={})\n{}", lang.name.to_uppercase(), lang.name, lang.usage));
      }
      text.push_str(&format!("\n\n{USAGE_FOOTER}"));
      text
    }
  };
  // Prints the page filled to the line width, as man would.
  out(&fill(&text));
  // Exits without running anything else.
  std::process::exit(0)
}

/// Writes a commented `./discover.conf` for the current directory, naming the detected language and every language's default entry points. An existing file is left alone.
pub fn write_config(langs: &[Language]) -> ExitCode {
  // Refuses to overwrite, because the file holds the user's choices.
  if Path::new("discover.conf").exists() {
    eprintln!("error: discover.conf already exists");
    return ExitCode::FAILURE;
  }
  // Reads the directory the file is keyed by.
  let cwd = match crate::project::current_dir() {
    Ok(cwd) => cwd.to_string_lossy().into_owned(),
    Err(e) => {
      eprintln!("error: {e}");
      return ExitCode::FAILURE;
    }
  };
  // Lists one comment line per language.
  let defaults: Vec<String> = langs.iter().map(|lang| format!("#   {}: {}", lang.name, lang.entry_defaults)).collect();
  // Leaves `lang` commented out when nothing identifies the project.
  let lang = match detect_lang(langs, &[]) {
    Some(lang) => format!("  lang: {lang}"),
    None => "  # lang: <language>".to_string(),
  };
  // Quotes the key as JSON, which is also valid YAML, so any path is safe.
  let key = serde_json::to_string(&cwd).expect("a string serializes");
  let text = format!(
    "# discover configuration: one block per project directory. JSON is also accepted.\n#\n# lang     Which navigator to run. Omit it to detect the language from the project.\n# entries  Repo-relative files where the program starts, the roots of liveness.\n#          Listing any replaces the defaults below completely, so list every entry point.\n#          DISCOVER_ENTRY=\"<file> ...\" overrides this list for a single run.\n#\n# Default entry points, used when `entries` is not set:\n{}\n{key}:\n{lang}\n  # entries:\n  #   - path/to/entry\n",
    defaults.join("\n")
  );
  // Writes the file and reports the outcome.
  match std::fs::write("discover.conf", text) {
    Ok(()) => {
      println!("wrote discover.conf");
      ExitCode::SUCCESS
    }
    Err(e) => {
      eprintln!("error: cannot write discover.conf: {e}");
      ExitCode::FAILURE
    }
  }
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
      // Turns on strict mode.
      "--strict" => STRICT.store(true, Ordering::Relaxed),
      // Turns on verbose mode.
      "-v" | "--verbose" => VERBOSE.store(true, Ordering::Relaxed),
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
  // Stores the subcommand's arguments.
  ARGS.set(args).expect("parse_args runs once");
}

fn run<L: Lang>(
  usage: &str,
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
  let command = SUBCOMMAND.get().map_or("", String::as_str);
  if command.is_empty() && !rebuild {
    // Prints the help page, filled to the line width: the shared part, the language's own commands, then the footer.
    eprintln!("{}", fill(&usage_page(L::NAME, usage)));
    return Ok(1);
  }

  let mut graph = open(rebuild)?;
  if rebuild {
    graph.build_all()?;
    // With no subcommand, the rebuild was the whole request.
    if command.is_empty() {
      return Ok(0);
    }
  }

  match command {
    "live-files" => {
      graph.live()?.iter().for_each(|l| out(l));
      Ok(0)
    }
    "dead-files" => {
      if args.first().is_some_and(|a| a == "--rebuild") {
        graph.build_dead()?;
      }
      graph.dead()?.iter().for_each(|l| out(l));
      Ok(0)
    }
    "is-live" => cmd_is_live(&graph, &args),
    "importers" => cmd_importers(&graph, &args),
    "trace" => cmd_trace(&graph, &args),
    "prune" => cmd_prune(&mut graph, &args),
    other => commands(&graph, other, &args),
  }
}

/// Returns the help page up to and including the commands every language shares: NAME, SYNOPSIS, DESCRIPTION, OPTIONS and the start of COMMANDS. A language's own commands follow it, then [`USAGE_FOOTER`]. Laid out like a manual page: section headings at the margin, text indented 7 columns, entry descriptions 14; paragraphs are single lines here, and [`fill`] breaks them to the line width when the page is printed.
fn shared_usage(name: &str) -> String {
  // Fills the command, with its --lang= when one is known, into the synopsis.
  format!(
    "NAME
       discover - navigate a codebase through its import graph

SYNOPSIS
       {name} [OPTION]... COMMAND [ARG]...
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
              For importers, trace and prune: make a FILE that would be skipped with a warning an error instead, so nothing runs.

       -v, --verbose
              Print more detail where a command supports it.

       -h, --help
              Print this help and exit. With a known language, only that language's commands are listed; otherwise every language's are.

       -V, --version
              Print the version and build time and exit.

COMMANDS
       config
              Write a commented ./discover.conf for the current directory, naming the detected language and every language's default entry points. An existing discover.conf is left alone. Must be the first argument.

       live-files
              List every live file, one per line.

       dead-files [--rebuild]
              List every dead file, one per line, found by the backward walk. --rebuild rebuilds the dead-files cache first.

       is-live FILE
              Print LIVE: FILE or DEAD: FILE.

       importers FILE...
              List the files that import each FILE, one per line. With more than one FILE, each line is TARGET:IMPORTER. A FILE that doesn't exist is skipped with a warning.

       trace [-v|--verbose] FILE...
              Print one JSON document per FILE: its entities, leaving out those private to it (in Dart, names starting with _) unless --verbose is given; its importers, each with the entities it uses and tagged active (a live importer that uses them), stale (a live importer that uses none) or zombie (a dead importer); and whether FILE is dead. A FILE that doesn't exist is skipped with a warning, and so, even with --strict, is one that isn't a source file.

       prune FILE...
              Remove deleted files from the caches, so the deletion doesn't trigger a rebuild on the next run. A FILE that still exists is skipped with a warning, and so, even with --strict, is one the caches don't mention."
  )
}

/// The end of the help page, after a language's own commands: ENVIRONMENT and FILES, which hold for every language.
const USAGE_FOOTER: &str = "ENVIRONMENT
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

/// The whole help page for one language: the shared part, the language's own commands, then the footer.
fn usage_page(name: &str, usage: &str) -> String {
  format!("{}\n\n{usage}\n\n{USAGE_FOOTER}", shared_usage(name))
}

/// Fills a help page to the line width the way man(1) renders a page: each line keeps its leading indent, and one longer than the width is broken between words into lines that repeat that indent. The width is $MANWIDTH, else $COLUMNS, else 80, as man uses.
fn fill(text: &str) -> String {
  // Reads the width from the environment, falling back to 80 columns.
  let width = ["MANWIDTH", "COLUMNS"]
    .iter()
    .find_map(|var| std::env::var(var).ok().and_then(|value| value.parse::<usize>().ok()))
    .filter(|&width| width > 0)
    .unwrap_or(80);
  // Holds the filled lines.
  let mut filled = Vec::new();
  for line in text.lines() {
    // Keeps a line that already fits exactly as written.
    if line.chars().count() <= width {
      filled.push(line.to_string());
      continue;
    }
    // Splits off the indent, which every continuation line repeats.
    let body = line.trim_start();
    let indent = &line[..line.len() - body.len()];
    // Holds the line being filled and whether it has a word on it yet.
    let mut current = indent.to_string();
    let mut has_word = false;
    for word in body.split_whitespace() {
      // Starts a new line when the word would run past the width; a word longer than the width gets a line to itself.
      if has_word && current.chars().count() + 1 + word.chars().count() > width {
        filled.push(std::mem::replace(&mut current, indent.to_string()));
        has_word = false;
      }
      // Separates the word from the one before it on the same line.
      if has_word {
        current.push(' ');
      }
      current.push_str(word);
      has_word = true;
    }
    filled.push(current);
  }
  filled.join("\n")
}

/// Returns the strings whose existence as a file equals `constraint`, and prints a warning containing `msg` for each of the others.
fn collect_files(constraint: bool, msg: &str, strings: &[String]) -> Vec<String> {
  // Holds the strings that satisfy the constraint.
  let mut files = Vec::with_capacity(strings.len());
  // Builds the current working directory prefix, or None when it can't be read.
  let cwd = std::env::current_dir().ok().map(|dir| format!("{}/", dir.display()));
  // Visits every string, in argument order.
  for maybe_file in strings {
    // Removes a leading `./`, or else a leading working directory, leaving any other string as it is.
    let stripped = maybe_file
      // Tries the `./` prefix first.
      .strip_prefix("./")
      // Tries the working directory prefix when `./` wasn't there.
      .or_else(|| maybe_file.strip_prefix(cwd.as_deref()?))
      // Falls back to the string as given.
      .unwrap_or(maybe_file);
    // Compares whether the path is a file with the constraint.
    if Path::new(stripped).is_file() == constraint {
      // Keeps an owned copy of the stripped form of a string that satisfies the constraint.
      files.push(stripped.to_string());
    } else {
      // Warns about a string that doesn't satisfy the constraint.
      eprintln!("warning: {maybe_file} {msg}");
    }
  }
  // Returns the strings that were kept.
  files
}

/// Returns the same strings as `collect_files`, or an error if any string failed the constraint.
fn collect_files_strict(constraint: bool, msg: &str, strings: &[String]) -> Result<Vec<String>, String> {
  // Filters the strings, which prints a warning for every failure.
  let files = collect_files(constraint, msg, strings);
  // Detects a failure by comparing how many strings were kept with how many were given.
  if files.len() != strings.len() {
    // Returns the error, with a hint saying how to disable the constraint.
    return Err("--strict: nothing was run\nhint: run without --strict to skip the paths above and continue with the rest".into());
  }
  // Returns the strings, all of which passed.
  Ok(files)
}

/// An existing file argument as (canonical path, repo-relative path).
pub fn file_arg(root: &Path, file: &str) -> Result<(PathBuf, String), String> {
  let abs = to_abs(root, file);
  if !abs.is_file() {
    return Err(format!("file not found: {file}"));
  }
  let canonical = std::fs::canonicalize(&abs).map_err(|e| format!("{file}: {e}"))?;
  let rel = relative(root, &canonical);
  Ok((canonical, rel))
}

// ─────────────────────────────────────────────────────────
// argument parsing for the language subcommands
// ─────────────────────────────────────────────────────────

/// `resolve <what> [--from <file>]` → (what, from); from is "" when absent.
/// `what` names the argument in the usage message (`<uri>`, `<import>`, …).
pub fn parse_resolve<'a, L: Lang>(args: &'a [String], what: &str) -> Result<(&'a str, &'a str), String> {
  let mut uri = "";
  let mut from = "";
  let mut it = args.iter();
  while let Some(arg) = it.next() {
    match arg.as_str() {
      "--from" => from = it.next().map_or("", String::as_str),
      other => uri = other,
    }
  }
  if uri.is_empty() {
    return Err(format!("usage: {} resolve {what} [--from <file>]", L::NAME));
  }
  Ok((uri, from))
}

pub struct InspectArgs<'a> {
  /// Whether each of the language's section flags was given, in order.
  pub sections: Vec<bool>,
  pub entities: bool,
  /// name | names | signatures | full
  pub depth: String,
  pub file: Option<&'a str>,
}

/// `inspect <file> [<section flags>] [--entities] [--depth names|signatures|full]`, in any order.
/// With no section and no --entities, everything is selected.
pub fn parse_inspect<'a>(args: &'a [String], section_flags: &[&str]) -> InspectArgs<'a> {
  let mut sections = vec![false; section_flags.len()];
  let mut entities = false;
  // Empty without --depth: entities print as JSON with kind, name and line.
  let mut depth = String::new();
  let mut file = None;
  let mut i = 0;
  while i < args.len() {
    let arg = args[i].as_str();
    if let Some(at) = section_flags.iter().position(|f| *f == arg) {
      sections[at] = true;
    } else {
      match arg {
        "--entities" => entities = true,
        "--depth" => {
          i += 1;
          depth = args.get(i).cloned().unwrap_or_default();
        }
        other => file = Some(other),
      }
    }
    i += 1;
  }
  if !(entities || sections.iter().any(|s| *s)) {
    sections.iter_mut().for_each(|s| *s = true);
    entities = true;
  }
  InspectArgs {
    sections,
    entities,
    depth,
    file,
  }
}

pub enum UsagesArgs<'a> {
  Pattern { pattern: &'a str, live_only: bool },
  Assets(&'a str),
}

/// `usages <pattern> [--live-only]` or `usages --assets <asset-path>`.
pub fn parse_usages<'a, L: Lang>(args: &'a [String]) -> Result<UsagesArgs<'a>, String> {
  let mut pattern = "";
  let mut asset = None;
  // Holds whether --live-only was given, keeping only matches in live files.
  let mut live_only = false;
  let mut it = args.iter();
  while let Some(arg) = it.next() {
    match arg.as_str() {
      "--live-only" => live_only = true,
      "--assets" => {
        asset = Some(
          it.next()
            .filter(|a| !a.is_empty())
            .ok_or("--assets requires an asset path argument")?
            .as_str(),
        )
      }
      other => pattern = other,
    }
  }
  if let Some(asset) = asset {
    return Ok(UsagesArgs::Assets(asset));
  }
  if pattern.is_empty() {
    let name = L::NAME;
    return Err(format!(
      "usage: {name} usages <pattern> [--live-only] [-v|--verbose]\n       {name} usages --assets <asset-path>"
    ));
  }
  Ok(UsagesArgs::Pattern { pattern, live_only })
}

/// Pattern results on stdout so they can be piped; the count is commentary.
pub fn print_usages(found: &[Usage]) {
  out_json(&found);
  eprintln!("{} usages found", found.len());
}

// ─────────────────────────────────────────────────────────
// output shapes shared by the language subcommands
// ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct EntityOut<'a> {
  pub kind: &'a str,
  pub name: &'a str,
  pub line: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub text: Option<&'a str>,
}

impl<'a> EntityOut<'a> {
  /// `text` is the header for signatures depth, the whole declaration for
  /// full depth, and omitted otherwise.
  pub fn new(kind: &'a str, name: &'a str, line: usize, header: &'a str, full: &'a str, depth: &str) -> Self {
    let text = match depth {
      "full" => Some(full),
      "signatures" => Some(header),
      _ => None,
    };
    EntityOut { kind, name, line, text }
  }
}

#[derive(Serialize)]
pub struct DefinitionOut<'a> {
  pub symbol: &'a str,
  pub kind: &'a str,
  pub file: &'a str,
  pub line: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub via: Option<&'a [String]>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source: Option<&'a str>,
  /// The whole declaration, with `--verbose`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub code: Option<&'a str>,
}

pub struct DefinitionArgs<'a> {
  pub symbol: &'a str,
  /// The file the symbol is used in; without it every definition in the
  /// project is reported.
  pub file: Option<&'a str>,
  /// `-v` / `--verbose`: include each definition's code.
  pub verbose: bool,
  /// `-p` / `--pretty`: print each definition as a header line and its code
  /// as written instead of JSON. Implies `verbose`.
  pub pretty: bool,
}

/// `definition <symbol> [<file>] [-v|--verbose] [-p|--pretty]`.
pub fn parse_definition<'a, L: Lang>(args: &'a [String]) -> Result<DefinitionArgs<'a>, String> {
  let usage = || format!("usage: {} definition <symbol> [<file>] [-v|--verbose] [-p|--pretty]", L::NAME);
  let mut verbose = VERBOSE.load(Ordering::Relaxed);
  let mut pretty = false;
  let mut positional = Vec::new();
  for arg in args {
    match arg.as_str() {
      "-p" | "--pretty" => pretty = true,
      other => positional.push(other),
    }
  }
  // Pretty output is the code, so it needs the code.
  verbose |= pretty;
  match positional[..] {
    [symbol] => Ok(DefinitionArgs { symbol, file: None, verbose, pretty }),
    [symbol, file] => Ok(DefinitionArgs { symbol, file: Some(file), verbose, pretty }),
    _ => Err(usage()),
  }
}

/// `--pretty` output for one definition: a `file:line kind symbol` header
/// (with the re-export chain, when there is one), then its code as written.
pub fn out_pretty_definition(def: &DefinitionOut) {
  let mut header = format!("{}:{} {} {}", def.file, def.line, def.kind, def.symbol);
  if let Some(via) = def.via.filter(|v| !v.is_empty()) {
    header.push_str(&format!(" (via {})", via.join(" -> ")));
  }
  out(&header);
  out(def.code.unwrap_or_default());
  out("");
}

/// One definition, as JSON or with `--pretty`.
pub fn out_definition(def: &DefinitionOut, pretty: bool) {
  if pretty {
    out_pretty_definition(def);
  } else {
    out_json(def);
  }
}

// ─────────────────────────────────────────────────────────
// is-live
// ─────────────────────────────────────────────────────────
fn cmd_is_live<L: Lang>(graph: &Graph<L>, args: &[String]) -> Result<u8, String> {
  let file = args.first().ok_or(format!("usage: {} is-live <file>", L::NAME))?;
  let (_, rel) = file_arg(graph.root(), file)?;
  if graph.live()?.contains(&rel) {
    out(&format!("LIVE: {rel}"));
    Ok(0)
  } else {
    out(&format!("DEAD: {rel}"));
    Ok(1)
  }
}

// ─────────────────────────────────────────────────────────
// importers
// ─────────────────────────────────────────────────────────
/// One importer per line. With several files, each line is `<target>:<importer>`, like grep with several files, so every importer stays attributed to its target.
fn cmd_importers<L: Lang>(graph: &Graph<L>, args: &[String]) -> Result<u8, String> {
  // Rejects a call with no file paths.
  if args.is_empty() {
    // Returns the usage message as the error.
    return Err(format!("usage: {} importers [--strict] <files ...>", L::NAME));
  }
  // Names the failure in the warning for a path that isn't a file.
  let msg = "does not exist";
  // Keeps the paths that are files, or errors on any that isn't when strict.
  let targets = if STRICT.load(Ordering::Relaxed) { collect_files_strict(true, msg, args)? } else { collect_files(true, msg, args) };
  // Loads the importers index once for all targets.
  let index = graph.importers()?;
  // Visits the targets in argument order.
  for file in &targets {
    // Gets this target's importer rows, or none when nothing imports it.
    let mut rows: Vec<&Row> = index.get(file).map(|rows| rows.iter().collect()).unwrap_or_default();
    // Sorts the rows by importer path.
    rows.sort_by(|a, b| a.file.cmp(&b.file));
    // Prints one line per importer.
    for row in rows {
      // Uses the argument count, so a skipped path can't change the output format.
      if args.len() > 1 {
        // Prefixes the target so each importer stays tied to it.
        out(&format!("{file}:{}", row.file));
      } else {
        // Prints the bare importer path for a single argument.
        out(&row.file);
      }
    }
  }
  // Exits successfully, including when paths were skipped.
  Ok(0)
}

// ─────────────────────────────────────────────────────────
// prune
// ─────────────────────────────────────────────────────────
fn cmd_prune<L: Lang>(graph: &mut Graph<L>, args: &[String]) -> Result<u8, String> {
  // Rejects a call with no file paths.
  if args.is_empty() {
    // Returns the usage message as the error.
    return Err(format!("usage: {} prune [--strict] <files ...>", L::NAME));
  }
  // Names the failure in the warning for a path that is still a file.
  let msg = "still exists";
  // Keeps the paths that are not files, or errors on any that is when strict.
  let targets = if STRICT.load(Ordering::Relaxed) { collect_files_strict(false, msg, args)? } else { collect_files(false, msg, args) };
  // Loads the set of paths the caches mention.
  let cached = graph.cached_files();
  // Visits the surviving paths in argument order.
  for file in &targets {
    // Prunes only a path the caches mention.
    if cached.contains(file) {
      // Removes the path from the caches.
      graph.prune(file)?;
    } else {
      // Warns about a path the caches don't mention, in strict mode too.
      eprintln!("warning: {file} is not in the caches");
    }
  }
  // Exits successfully, including when paths were skipped.
  Ok(0)
}

// ─────────────────────────────────────────────────────────
// trace — tags each importer of a file:
//   active — live importer that uses the target's entities
//   stale  — live importer that doesn't use any entities
//   zombie — dead importer (the dependency exists but the chain is dead)
// The file is dead when no importer is active, then the
// verdict is cross-checked against is-live.
// Several files produce one JSON document each, in argument
// order, which jq reads as a stream.
// ─────────────────────────────────────────────────────────
#[derive(Serialize)]
struct TracedRow {
  file: String,
  directive: String,
  uri: String,
  entities_used: Vec<EntityUse>,
  status: &'static str,
  /// Present (true) when the importer passes the target's entities on to
  /// its own importers (it exports, inherits from or mixes in the target);
  /// see [`Lang::is_transitive`].
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  transitive: bool,
}

#[derive(Serialize)]
// Borrows the file path for the moment the output is serialized.
struct TraceOut<'a> {
  // The target path, as given on the command line.
  file: &'a str,
  entities: Vec<String>,
  importers: Vec<TracedRow>,
  dead: bool,
  active_count: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  warning: Option<String>,
}

fn cmd_trace<L: Lang>(graph: &Graph<L>, args: &[String]) -> Result<u8, String> {
  // Rejects a call with no file paths.
  if args.is_empty() {
    // Returns the usage message as the error.
    return Err(format!("usage: {} trace [--strict] [-v|--verbose] <files ...>", L::NAME));
  }
  // Names the failure in the warning for a path that isn't a file.
  let msg = "does not exist";
  // Keeps the paths that are files, or errors on any that isn't when strict.
  let targets = if STRICT.load(Ordering::Relaxed) { collect_files_strict(true, msg, args)? } else { collect_files(true, msg, args) };
  // Loaded once and shared: the reachability memo stays valid across targets.
  let index = graph.importers()?;
  let live = graph.live()?;
  let mut trace = Trace::new(&index, graph.entries());

  // Visits the targets in argument order.
  for file in &targets {
    // Views the argument as a path for the file-based lookups.
    let path = Path::new(file);
    // Skips a file that trace can't follow.
    if !graph.is_source(path) {
      // Warns about a non-source file, in strict mode too, and points to the command for assets.
      eprintln!("warning: {file} is not a source file (trace is for code; for assets use: {} usages --assets <path>)", L::NAME);
      // Moves on to the next target.
      continue;
    }
    // Reads the names of the entities the file declares.
    let mut entities = graph.entity_names(path);
    // Drops the private ones, which no importer can use, unless -v/--verbose asks for every entity.
    if !VERBOSE.load(Ordering::Relaxed) {
      entities.retain(|name| !L::is_private(name));
    }
    // Builds a tagged row for each importer stored under this file in the index.
    let mut importers: Vec<TracedRow> = index
      // Looks up this file's importer rows, using the argument as the key.
      .get(file)
      // Treats the rows as a slice.
      .map(Vec::as_slice)
      .unwrap_or_default()
      .iter()
      .map(|row| {
        let status = if !trace.reachable(&row.file) {
          "zombie"
        } else if row.entities_used.is_empty() {
          "stale"
        } else {
          "active"
        };
        TracedRow {
          file: row.file.clone(),
          directive: row.directive.clone(),
          uri: row.uri.clone(),
          entities_used: row.entities_used.clone(),
          status,
          transitive: L::is_transitive(&row.directive),
        }
      })
      .collect();
    importers.sort_by(|a, b| a.file.cmp(&b.file));

    let active_count = importers.iter().filter(|r| r.status == "active").count();
    // An entry point has no importers by definition, yet is live.
    let dead = active_count == 0 && !graph.entries().contains(file);
    // Reads the file's dead verdict from the live list, for the cross-check below.
    let bfs_dead = !live.contains(file);
    let warning = (dead != bfs_dead).then(|| {
      let w = format!("DISAGREEMENT: trace says dead={dead} but is-live says dead={bfs_dead}");
      eprintln!("WARNING: {w}");
      w
    });

    out_json(&TraceOut {
      // Passes the target path without copying it.
      file,
      entities,
      importers,
      dead,
      active_count,
      warning,
    });
  }
  // Exits successfully, including when paths were skipped.
  Ok(0)
}
