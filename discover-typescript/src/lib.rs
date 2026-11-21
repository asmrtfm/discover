//! discover-typescript — deterministic TypeScript and JavaScript codebase navigation,
//! in-process on ast-grep's TypeScript/TSX support. A port of the
//! TypeScript discover.sh: same subcommands, flags, JSON shapes and stderr
//! commentary, with each file parsed once instead of once per query. The
//! subcommands that only need the graph come from discover-core; the
//! TypeScript-specific ones are here.
//!
//! Subcommands:
//!   resolve <specifier> [--from <file>]
//!   inspect <file> [--imports] [--exports] [--dynamic] [--entities] [--depth names|signatures|full]
//!   definition <file> <symbol>
//!   live-files
//!   dead-files [--rebuild]
//!   is-live <file>
//!   importers [--strict] <files ...>
//!   trace [--strict] <files ...>
//!   prune [--strict] <files ...>
//!   usages <pattern> [--live-only]
//!   usages --assets <asset-path>
//!   attributes <class> <file>

mod commands;
mod graph;
mod help;
pub mod project;
pub mod ts;

use commands::{attributes, definition, inspect, resolve, usages};
use discover_core::cli;
use discover_core::help::Help;
use help::TypeScriptHelp;
use project::Project;
use std::process::ExitCode;

/// How the discover binary identifies and runs TypeScript.
pub const LANGUAGE: cli::Language = cli::Language {
  // Selects TypeScript with --lang=typescript.
  name: "typescript",
  // Identifies TypeScript and JavaScript source files.
  extensions: &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"],
  // Identifies a TypeScript or JavaScript project.
  manifests: &["package.json"],
  // Describes the default entry points in generated config files.
  entry_defaults: "package.json source/module/main/bin, node commands in package.json scripts, Dockerfile CMD/ENTRYPOINT and docker-compose command, else the first of index, server, app, main at the root or in src/",
  // Runs the function below.
  main,
  // Lists the subcommands TypeScript implements, for --help.
  commands: TypeScriptHelp::COMMANDS,
  // Gives TypeScript's page for a subcommand, for --help.
  page: discover_core::help::page::<TypeScriptHelp>,
};

/// Runs the TypeScript navigator.
pub fn main() -> ExitCode {
  cli::main(|_| Ok(graph::open(Project::discover()?)), |graph, command, args| {
    match command {
      "resolve" => resolve::cmd_resolve(graph, args),
      "inspect" => inspect::cmd_inspect(graph, args),
      "definition" => definition::cmd_definition(graph, args),
      "usages" => usages::cmd_usages(graph, args),
      "attributes" => attributes::cmd_attributes(graph, args),
      // endpoints is Dart only for now: no output, exit 1.
      "endpoints" => Ok(1),
      _ => unreachable!("checked against COMMANDS"),
    }
  })
}
