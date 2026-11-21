use discover_core::help::{Page, inspect as core};

/// The JVM's `inspect`: its sections are the package and imports.
pub fn inspect() -> Page {
  Page {
    synopsis: &["inspect FILE [--package] [--imports] [--entities] [--depth names|signatures|full]"],
    description: &["Print FILE's package, imports and top-level entities as JSON; with none of these flags, all of them."],
    options: &[
      ("--package, --imports", "Print FILE's package or imports."),
      ("--entities", "Print the top-level entities."),
      ("--depth names|signatures|full", "--depth signatures adds each entity's header line and --depth full its whole declaration; --entities --depth names prints only the entity names, one per line."),
    ],
    ..core::inspect()
  }
}
