//! Dart's help: every subcommand, with its own page where Dart's implementation has something specific to say.

mod definition;
mod endpoints;
mod inspect;
mod resolve;
mod trace;
mod usages;

use discover_core::help::{Help, Page};

pub struct DartHelp;

impl Help for DartHelp {
  const COMMANDS: &'static [&'static str] = &["resolve", "inspect", "definition", "usages", "attributes", "endpoints", "importers", "trace", "prune", "dead-files", "is-live"];

  fn resolve() -> Page {
    resolve::resolve()
  }
  fn inspect() -> Page {
    inspect::inspect()
  }
  fn definition() -> Page {
    definition::definition()
  }
  fn usages() -> Page {
    usages::usages()
  }
  fn endpoints() -> Page {
    endpoints::endpoints()
  }
  fn trace() -> Page {
    trace::trace()
  }
}
