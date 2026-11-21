//! The JVM languages' help, shared by Java and Kotlin: every subcommand they implement, with its own page where the JVM implementation has something specific to say.

mod definition;
mod inspect;
mod resolve;

use discover_core::help::{Help, Page};

pub struct JvmHelp;

impl Help for JvmHelp {
  const COMMANDS: &'static [&'static str] = &["resolve", "inspect", "definition", "usages", "importers", "trace", "prune", "dead-files", "is-live"];

  fn resolve() -> Page {
    resolve::resolve()
  }
  fn inspect() -> Page {
    inspect::inspect()
  }
  fn definition() -> Page {
    definition::definition()
  }
}
