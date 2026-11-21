use discover_core::help::{Page, definition as core};

/// TypeScript's `definition`: it takes FILE first and requires it, with no flags.
pub fn definition() -> Page {
  Page {
    synopsis: &["definition FILE SYMBOL"],
    description: &["Print where SYMBOL, as used in FILE, is defined."],
    options: &[],
    ..core::definition()
  }
}
