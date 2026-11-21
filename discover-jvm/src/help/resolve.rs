use discover_core::help::{Page, resolve as core};

/// The JVM's `resolve`: an import names declarations, so it can resolve to several files.
pub fn resolve() -> Page {
  Page {
    synopsis: &["resolve IMPORT [--from FILE]"],
    description: &["Print the file or files declaring IMPORT (com.example.Foo or com.example.*)."],
    options: &[("--from FILE", "IMPORT is a simple name, resolved as FILE sees it.")],
    ..core::resolve()
  }
}
