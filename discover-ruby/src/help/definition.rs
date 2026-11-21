use discover_core::help::{Page, definition as core};

/// Ruby's `definition`: the order it searches in, and the names it takes.
pub fn definition() -> Page {
  Page {
    description: &["Print where SYMBOL, as used in FILE, is defined: among FILE's own entities, then the file Zeitwerk autoloads it from, then the files FILE requires or inherits from (superclasses and mixins), directly or through them, nearest first. SYMBOL is a name (User, valid?) or a qualified name (Admin::User, User#valid?, User.find_by_slug). Without FILE, print every definition of SYMBOL in the project."],
    ..core::definition()
  }
}
