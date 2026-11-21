use discover_core::help::{Page, trace as core};

/// Ruby's `trace`, which replaces core's: where uses are looked for, and its own options.
pub fn trace() -> Page {
  Page {
    synopsis: &["trace [--include-upstream] FILE..."],
    description: &[
      "Print one JSON document per FILE: its entities; its importers, each with the entities it uses and tagged active (a live importer that uses them), stale (a live importer that uses none) or zombie (a dead importer); and whether FILE is dead.",
      "For each importer, entities_used lists where FILE's entities are used through it: in the importer itself; when the importer is a module, in every class and module downstream of it (those that include, extend, prepend or inherit it, through any number of such links); and when the importer or anything downstream of it is a class, at every callsite of that class in the codebase, ERB templates included. An entry found in another file has that file and a via list, the inheritance chain from the importer to it; each entry is printed on one line. A row is stale only when none of these finds a use. The constant named in a superclass or include/extend/prepend statement is not a use.",
    ],
    options: &[
      ("--include-upstream", "Also count the entities FILE pulls in through its own requires, superclasses and mixins."),
      ("-v, --verbose", "Also list every file reached through transitive links (requires, superclasses, mixins) as a row of its own, with a via list of the files between it and FILE, nearest FILE first."),
    ],
    ..core::trace()
  }
}
