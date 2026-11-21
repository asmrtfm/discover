use super::Page;

/// `resolve`'s generic page.
pub fn resolve() -> Page {
  Page {
    summary: "print the file an import resolves to",
    synopsis: &["resolve SPECIFIER [--from FILE]"],
    description: &["Print the repo-relative path of the file an import SPECIFIER resolves to."],
    options: &[("--from FILE", "Resolve SPECIFIER as FILE sees it.")],
  }
}
