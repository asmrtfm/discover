use discover_core::help::{Page, usages as core};

/// TypeScript's `usages`: each file is parsed with its own grammar, and assets are found through imports, require() and public/ URLs.
pub fn usages() -> Page {
  Page {
    description: &["Print every match of the ast-grep PATTERN in the sources as JSON, each file parsed with its own grammar."],
    options: &[("--assets PATH", "Print the references to the asset at PATH as JSON instead: imports and require() calls of it, and URL strings for files under public/.")],
    ..core::usages()
  }
}
