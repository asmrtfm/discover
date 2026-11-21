//! Ruby's help: every subcommand it implements, with its own page where Ruby's implementation has something specific to say.

mod attributes;
mod definition;
mod importers;
mod inspect;
mod resolve;
mod trace;
mod usages;

use discover_core::help::{Help, Page};

pub struct RubyHelp;

impl Help for RubyHelp {
  const COMMANDS: &'static [&'static str] = &["resolve", "inspect", "definition", "usages", "attributes", "importers", "trace", "prune", "dead-files", "is-live"];

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
  fn attributes() -> Page {
    attributes::attributes()
  }
  fn importers() -> Page {
    importers::importers()
  }
  fn trace() -> Page {
    trace::trace()
  }
}
