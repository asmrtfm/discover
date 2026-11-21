//! discover-ruby — deterministic Ruby codebase navigation, in-process on ast-grep's Ruby support. Files link through require, require_relative, load and autoload, each resolved by its own rule; in projects that use Zeitwerk (Rails apps, gems calling `Zeitwerk::Loader.for_gem`), a reference to an autoloaded constant links to the file that defines it. The subcommands that only need the graph come from discover-core; the Ruby-specific ones are here.
//!
//! Subcommands:
//!   resolve <specifier|constant> [--from <file>] [--require|--require-relative|--load|--autoload]
//!   inspect <file> [--requires] [--ancestors] [--constants] [--entities] [--depth names|signatures|full]
//!   definition <symbol> [<file>] [-v|--verbose] [-p|--pretty]
//!   live-files
//!   dead-files [--rebuild]
//!   is-live <file>
//!   importers [--strict] <files ...>
//!   trace [--strict] [--include-upstream] [-v|--verbose] <files ...>
//!   prune [--strict] <files ...>
//!   usages <pattern> [--live-only]
//!   usages --assets <asset-path>
//!   attributes <class> <file> [-v|--verbose]

mod commands;
mod common;
mod graph;
mod help;
pub mod project;
pub mod rb;

use commands::{attributes, definition, inspect, resolve, trace, usages};
use discover_core::cli;
use discover_core::help::Help;
use help::RubyHelp;
use project::Project;
use std::process::ExitCode;

/// How the discover binary identifies and runs Ruby.
pub const LANGUAGE: cli::Language = cli::Language {
  // Selects Ruby with --lang=ruby.
  name: "ruby",
  // Identifies Ruby source files.
  extensions: &["rb", "rake"],
  // Identifies a Ruby project.
  manifests: &["Gemfile", "gems.rb"],
  // Describes the default entry points in generated config files.
  entry_defaults: "Ruby executables in bin/ and exe/, lib/<name>.rb for each <name>.gemspec, Rakefile, config.ru and lib/tasks/**/*.rake; in Rails also config/**, db/**, controllers, channels, helpers, mailboxes and templates under app/; else the first of main.rb, app.rb, lib/main.rb, lib/app.rb",
  // Runs the function below.
  main,
  // Lists the subcommands Ruby implements, for --help.
  commands: RubyHelp::COMMANDS,
  // Gives Ruby's page for a subcommand, for --help.
  page: discover_core::help::page::<RubyHelp>,
};

/// Runs the Ruby navigator.
pub fn main() -> ExitCode {
  cli::main(|_| Ok(graph::open(Project::discover()?)), |graph, command, args| match command {
    "resolve" => resolve::cmd_resolve(graph, args),
    "inspect" => inspect::cmd_inspect(graph, args),
    "definition" => definition::cmd_definition(graph, args),
    "usages" => usages::cmd_usages(graph, args),
    "attributes" => attributes::run(graph, args),
    // Ruby's own trace, which replaces core's (see RbLang::OVERRIDES).
    "trace" => trace::cmd_trace(graph, args),
    // endpoints is Dart only for now: no output, exit 1.
    "endpoints" => Ok(1),
    _ => unreachable!("checked against COMMANDS"),
  })
}
