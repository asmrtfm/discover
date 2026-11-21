use discover_core::help::{Page, usages as core};

/// Ruby's `usages`: how an asset is named in Ruby and ERB.
pub fn usages() -> Page {
  Page {
    options: &[("--assets PATH", "Print the lines of live Ruby files and ERB templates whose string literals name the asset at PATH instead: its repo path, its URL for a file under public/, or its logical path for a file under app/assets/<dir>/ (what image_tag takes).")],
    ..core::usages()
  }
}
