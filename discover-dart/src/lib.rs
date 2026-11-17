//! discover-dart — deterministic Dart codebase navigation, in-process on
//! ast-grep's Dart support. A port of discover.sh: same subcommands, flags,
//! JSON shapes and stderr commentary, with each file parsed once instead of
//! once per query. The subcommands that only need the graph come from
//! discover-core; the Dart-specific ones are here.
//!
//! Subcommands:
//!   resolve <uri> [--from <file>]
//!   inspect <file> [--imports] [--exports] [--parts] [--entities] [--depth names|signatures|full]
//!   definition <symbol> [<file>] [-v|--verbose] [-p|--pretty]
//!   live-files
//!   dead-files [--rebuild]
//!   is-live <file>
//!   importers [--strict] <files ...>
//!   trace [--strict] [-v|--verbose] <files ...>
//!   prune [--strict] <files ...>
//!   usages <pattern> [--live-only] [-v|--verbose]
//!   usages --assets <asset-path>
//!   attributes <class> <file>
//!   endpoints [<url-regex> ...] [--all] [-v|--verbose]

mod attributes;
mod common;
pub mod dart;
mod definition;
pub mod endpoints;
mod graph;
mod project;
mod usages;

use dart::Directive;
use discover_core::cli::{
  self, EntityOut, UsagesArgs, file_arg, out, out_json, parse_inspect, parse_resolve, parse_usages,
};
use graph::{DartLang, Graph};
use project::Project;
use serde::Serialize;
use std::process::ExitCode;

/// Dart's own commands, continuing the help page's COMMANDS section in its manual-page layout.
const USAGE: &str = "       resolve URI [--from FILE]
              Print the repo-relative path a directive URI resolves to, relative to FILE when given.

       inspect FILE [--imports] [--exports] [--parts] [--entities] [--depth names|signatures|full]
              Print FILE's imports, exports, parts and top-level entities as JSON; with none of these flags, all of them. --depth signatures adds each entity's header line and --depth full its whole declaration; --entities --depth names prints only the entity names, one per line.

       definition SYMBOL [FILE] [-p|--pretty]
              Print where SYMBOL, as used in FILE, is defined: among FILE's own entities, then its imports (following re-exports), then its parts. Without FILE, print every definition of SYMBOL in the project, generated files included. --verbose adds each definition's code; --pretty prints a FILE:LINE KIND SYMBOL header followed by the code as written, instead of JSON.

       usages PATTERN [--live-only]
              Print every match of the ast-grep PATTERN under lib/ as JSON; with --live-only, only matches in live files. The symbol's own definitions are left out unless --verbose is given.

       usages --assets PATH
              Print the live references to the asset at PATH as JSON: through static String constants that hold it, and as string literals.

       attributes CLASS FILE
              List the instance fields of CLASS, declared in FILE, one name per line; with --verbose, a JSON array of { FIELD: TYPE } objects.

       endpoints [REGEX]... [--all] [-v|--verbose]
              Print URL FILE for each API request whose URL, as written in the source, matches any REGEX (unanchored), where FILE is the file the request is made in; with no REGEX, every request. With --verbose, print a line for every file on the request's call stack. Requests are Retrofit endpoint annotations, prefixed with their client's baseUrl; string literals starting with an API root ($rootURL, $postsRoute, $...Root or /api/); and absolute http(s) URLs passed to get, post, put, patch or delete. Only live files are walked; --all walks dead files too.";

/// How the discover binary identifies and runs Dart.
pub const LANGUAGE: cli::Language = cli::Language {
  // Selects Dart with --lang=dart.
  name: "dart",
  // Identifies Dart source files.
  extensions: &["dart"],
  // Identifies a Dart project.
  manifests: &["pubspec.yaml"],
  // Describes the default entry points in generated config files.
  entry_defaults: "lib/main.dart",
  // Runs the function below.
  main,
  // Lists the Dart commands for --help.
  usage: USAGE,
};

/// Runs the Dart navigator.
pub fn main() -> ExitCode {
  cli::main(USAGE, |_| graph::open(Project::discover()?), |graph, command, args| {
    match command {
      "resolve" => cmd_resolve(graph, args),
      "inspect" => cmd_inspect(graph, args),
      "definition" => definition::cmd_definition(graph, args),
      "usages" => cmd_usages(graph, args),
      "attributes" => attributes::cmd_attributes(graph, args),
      "endpoints" => cmd_endpoints(graph, args),
      _ => unreachable!("checked against COMMANDS"),
    }
  })
}

// ─────────────────────────────────────────────────────────
// resolve
// ─────────────────────────────────────────────────────────
fn cmd_resolve(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let (uri, from) = parse_resolve::<DartLang>(args, "<uri>")?;
  if let Some(resolved) = graph.project.resolve_uri(uri, from) {
    out(&resolved);
  }
  Ok(0)
}

// ─────────────────────────────────────────────────────────
// endpoints — `<url> <file>` for the file each selected
// API request is made in, or with --verbose every file on
// its call stack
// ─────────────────────────────────────────────────────────
fn cmd_endpoints(graph: &Graph, args: &[String]) -> Result<u8, String> {
  // --all walks dead files too; every other argument is a regex that selects URLs.
  let all = args.iter().any(|a| a == "--all");
  let filters: Vec<String> = args.iter().filter(|a| *a != "--all").cloned().collect();
  // -v/--verbose is global, so cli has already consumed it.
  let verbose = cli::VERBOSE.load(std::sync::atomic::Ordering::Relaxed);
  let lines = endpoints::endpoints(graph, &filters, all, verbose)?;
  lines.iter().for_each(|l| out(l));
  Ok(0)
}

// ─────────────────────────────────────────────────────────
// inspect
// ─────────────────────────────────────────────────────────
#[derive(Serialize)]
struct DirectiveOut {
  uri: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  resolved: Option<String>,
}

#[derive(Serialize)]
struct InspectOut<'a> {
  file: &'a str,
  #[serde(skip_serializing_if = "Option::is_none")]
  imports: Option<Vec<DirectiveOut>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  exports: Option<Vec<DirectiveOut>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  parts: Option<Vec<DirectiveOut>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  entities: Option<Vec<EntityOut<'a>>>,
}

fn cmd_inspect(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let parsed = parse_inspect(args, &["--imports", "--exports", "--parts"]);
  let file = parsed.file.ok_or(
    "usage: discover-dart inspect <file> [--imports] [--exports] [--parts] [--entities] [--depth names|signatures|full]",
  )?;
  let [imports, exports, parts] = parsed.sections[..] else {
    unreachable!("one flag per section")
  };
  let (entities, depth) = (parsed.entities, parsed.depth.as_str());
  let (path, rel) = file_arg(&graph.project.root, file)?;
  let facts = graph.facts(&path);

  // --depth names: plain text, one name per line, no JSON wrapper.
  if entities && depth == "names" {
    facts.entities.iter().for_each(|e| out(&e.name));
    return Ok(0);
  }

  let directive = |d: Directive| {
    facts
      .uris(d)
      .iter()
      .map(|uri| DirectiveOut {
        uri: uri.clone(),
        resolved: graph.project.resolve_uri(uri, &rel),
      })
      .collect::<Vec<_>>()
  };
  out_json(&InspectOut {
    file: &rel,
    imports: imports.then(|| directive(Directive::Import)),
    exports: exports.then(|| directive(Directive::Export)),
    parts: parts.then(|| directive(Directive::Part)),
    entities: entities.then(|| {
      facts
        .entities
        .iter()
        .map(|e| EntityOut::new(e.kind, &e.name, e.line, &e.header, &e.full, depth))
        .collect()
    }),
  });
  Ok(0)
}

// ─────────────────────────────────────────────────────────
// usages
// ─────────────────────────────────────────────────────────
fn cmd_usages(graph: &Graph, args: &[String]) -> Result<u8, String> {
  match parse_usages::<DartLang>(args)? {
    UsagesArgs::Assets(asset) => {
      let refs = usages::asset_usages(graph, asset)?;
      out_json(&refs);
      eprintln!(
        "{} constant ref(s), {} direct ref(s)",
        refs.constant_refs.len(),
        refs.direct_refs.len()
      );
    }
    UsagesArgs::Pattern { pattern, live_only } => {
      cli::print_usages(&usages::pattern_usages(graph, pattern, live_only)?);
    }
  }
  Ok(0)
}
