use discover_core::help::{Page, attributes as core};

/// TypeScript's `attributes`: CLASS can also be a Mongoose model.
pub fn attributes() -> Page {
  Page {
    description: &["List the instance fields of CLASS, declared in FILE, one name per line. CLASS can also be a Mongoose model, whose schema fields are listed."],
    ..core::attributes()
  }
}
