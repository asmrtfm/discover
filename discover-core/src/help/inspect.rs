use super::Page;

/// `inspect`'s generic page.
pub fn inspect() -> Page {
  Page {
    summary: "print a file's links and top-level entities",
    synopsis: &["inspect FILE [SECTION]... [--entities] [--depth names|signatures|full]"],
    description: &["Print FILE's sections and top-level entities as JSON; with none of these flags, all of them."],
    options: &[
      ("SECTION", "A flag naming one kind of link the language records for a file, such as --imports or --exports."),
      ("--entities", "Print the top-level entities."),
      ("--depth names|signatures|full", "--depth signatures adds each entity's header line and --depth full its whole declaration; --entities --depth names prints only the entity names, one per line."),
    ],
  }
}
