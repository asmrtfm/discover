use discover_core::help::{Page, definition as core};

/// Dart's `definition`: the order it searches in, and generated files counting without FILE.
pub fn definition() -> Page {
  Page {
    description: &["Print where SYMBOL, as used in FILE, is defined: among FILE's own entities, then its imports (following re-exports), then its parts. Without FILE, print every definition of SYMBOL in the project, generated files included."],
    ..core::definition()
  }
}
