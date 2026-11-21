use super::Page;

/// `importers`'s generic page.
pub fn importers() -> Page {
  Page {
    summary: "list the files that import a file",
    synopsis: &["importers FILE..."],
    description: &["List the files that import each FILE, one per line. With more than one FILE, each line is TARGET:IMPORTER."],
    options: &[],
  }
}
