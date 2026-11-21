//! TypeScript's help: every subcommand it implements, with its own page where TypeScript's implementation has something specific to say.

mod attributes;
mod definition;
mod inspect;
mod usages;

use discover_core::help::{Help, Page};

pub struct TypeScriptHelp;

impl Help for TypeScriptHelp {
  const COMMANDS: &'static [&'static str] = &["resolve", "inspect", "definition", "usages", "attributes", "importers", "trace", "prune", "dead-files", "is-live"];

  fn inspect() -> Page {
    inspect::inspect()
  }
  fn definition() -> Page {
    definition::definition()
  }
  fn usages() -> Page {
    usages::usages()
  }
  fn attributes() -> Page {
    attributes::attributes()
  }
}
