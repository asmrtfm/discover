use discover_core::help::{Page, inspect as core};

/// TypeScript's `inspect`: its sections are imports, exports and dynamic imports.
pub fn inspect() -> Page {
  Page {
    synopsis: &["inspect FILE [--imports] [--exports] [--dynamic] [--entities] [--depth names|signatures|full]"],
    description: &["Print FILE's imports, exports, dynamic imports and top-level entities as JSON; with none of these flags, all of them."],
    options: &[
      ("--imports, --exports, --dynamic", "Print FILE's imports, exports or dynamic imports."),
      ("--entities", "Print the top-level entities."),
      ("--depth names|signatures|full", "--depth signatures adds each entity's header line and --depth full its whole declaration; --entities --depth names prints only the entity names, one per line."),
    ],
    ..core::inspect()
  }
}
