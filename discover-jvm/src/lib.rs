//! The JVM navigators' shared half. Java and Kotlin resolve names the same
//! way — imports name declarations, not files, and a package's
//! declarations are visible without importing — and start from the same
//! entry points, so everything but reading a parse tree lives here. A
//! language crate implements [`Jvm`] and calls [`main`].

mod commands;
mod common;
pub mod graph;
pub mod help;
pub mod jvm;
pub mod project;

use commands::{definition, inspect, resolve, usages};
use discover_core::cli;
use project::Project;
use std::process::ExitCode;

// Keeps `discover_jvm::{Jvm, Facts, Decl, Import, Root, parse, ...}` working for the Java and Kotlin crates.
pub use jvm::*;

/// Runs the navigator for language `J`.
pub fn main<J: Jvm>() -> ExitCode {
  // The index underlies every lookup, entry detection included, so it is
  // brought up to date (or rebuilt for --rebuild-cache) before anything else.
  cli::main(
    |rebuild| graph::open(Project::<J>::discover()?, rebuild),
    |graph, command, args| match command {
      "resolve" => resolve::cmd_resolve(graph, args),
      "inspect" => inspect::cmd_inspect(graph, args),
      "definition" => definition::cmd_definition(graph, args),
      "usages" => usages::cmd_usages(graph, args),
      // attributes is Dart only for now: no output, exit 1.
      "attributes" => Ok(1),
      // endpoints is Dart only for now: no output, exit 1.
      "endpoints" => Ok(1),
      _ => unreachable!("checked against COMMANDS"),
    },
  )
}
