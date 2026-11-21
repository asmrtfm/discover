use super::Page;

/// `dead-files`'s generic page.
pub fn dead_files() -> Page {
  Page {
    summary: "list every dead file",
    synopsis: &["dead-files"],
    description: &["List every dead file, one per line, found by the backward walk."],
    options: &[],
  }
}
