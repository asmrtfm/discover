use super::Page;

/// `trace`'s generic page.
pub fn trace() -> Page {
  Page {
    summary: "classify each importer of a file",
    synopsis: &["trace FILE..."],
    description: &["Print one JSON document per FILE: its entities, leaving out those private to it; its importers, each with the entities it uses and tagged active (a live importer that uses them), stale (a live importer that uses none) or zombie (a dead importer); and whether FILE is dead."],
    options: &[("-v, --verbose", "Include the entities private to FILE.")],
  }
}
