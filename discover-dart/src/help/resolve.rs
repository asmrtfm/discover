use discover_core::help::{Page, resolve as core};

/// Dart's `resolve`: it resolves a directive URI.
pub fn resolve() -> Page {
  Page {
    synopsis: &["resolve URI [--from FILE]"],
    description: &["Print the repo-relative path a directive URI resolves to."],
    options: &[("--from FILE", "Resolve URI relative to FILE.")],
    ..core::resolve()
  }
}
