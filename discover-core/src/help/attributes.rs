use super::Page;

/// `attributes`'s generic page.
pub fn attributes() -> Page {
  Page {
    summary: "list the instance fields of a class",
    synopsis: &["attributes CLASS FILE"],
    description: &["List the instance fields of CLASS, declared in FILE, one name per line."],
    options: &[("-v, --verbose", "Print a JSON array of { FIELD: TYPE } objects instead.")],
  }
}
