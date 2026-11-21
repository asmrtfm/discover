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

mod commands;
mod common;
pub mod dart;
mod graph;
mod help;
mod project;

use commands::{attributes, definition, endpoints, inspect, resolve, usages};
use discover_core::cli;
use discover_core::help::Help;
use help::DartHelp;
use project::Project;
use std::process::ExitCode;

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
  // Lists the subcommands Dart implements, for --help.
  commands: DartHelp::COMMANDS,
  // Gives Dart's page for a subcommand, for --help.
  page: discover_core::help::page::<DartHelp>,
};

/// Runs the Dart navigator.
pub fn main() -> ExitCode {
  cli::main(|_| graph::open(Project::discover()?), |graph, command, args| {
    match command {
      "resolve" => resolve::cmd_resolve(graph, args),
      "inspect" => inspect::cmd_inspect(graph, args),
      "definition" => definition::cmd_definition(graph, args),
      "usages" => usages::cmd_usages(graph, args),
      "attributes" => attributes::cmd_attributes(graph, args),
      "endpoints" => endpoints::cmd_endpoints(graph, args),
      _ => unreachable!("checked against COMMANDS"),
    }
  })
}
