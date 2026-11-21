use discover_core::help::{Page, inspect as core};

/// Ruby's `inspect`: its sections are requires, ancestors and constants.
pub fn inspect() -> Page {
  Page {
    synopsis: &["inspect FILE [--requires] [--ancestors] [--constants] [--entities] [--depth names|signatures|full]"],
    description: &["Print FILE's require, require_relative, load and autoload calls, its superclasses and include/extend/prepend mixins, the constants it references, and its modules, classes, methods and constants (at any depth) as JSON; with none of these flags, all of them, constants only in projects that autoload."],
    options: &[
      ("--requires, --ancestors, --constants", "Print FILE's require-style calls, its superclasses and mixins, or the constants it references."),
      ("--entities", "Print the modules, classes, methods and constants."),
      ("--depth names|signatures|full", "--depth signatures adds each entity's header line and --depth full its whole declaration; --entities --depth names prints only the entity names, one per line."),
    ],
    ..core::inspect()
  }
}
