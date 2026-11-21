use super::Page;

/// `usages`'s generic page.
pub fn usages() -> Page {
  Page {
    summary: "find matches of an ast-grep pattern, or references to an asset",
    synopsis: &["usages PATTERN", "usages --assets PATH"],
    description: &["Print every match of the ast-grep PATTERN in the sources as JSON."],
    options: &[("--assets PATH", "Print the references to the asset at PATH in live code as JSON instead.")],
  }
}
