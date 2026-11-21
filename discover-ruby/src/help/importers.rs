use discover_core::help::{Page, importers as core};

/// Ruby's `importers`: what counts as a link between Ruby files.
pub fn importers() -> Page {
  Page {
    description: &[
      "List the files that import each FILE, one per line. With more than one FILE, each line is TARGET:IMPORTER.",
      "Links are require, require_relative, load and autoload calls whose path is a string literal, File.expand_path(\"x\", __dir__) or File.join(__dir__, \"x\"), and superclasses and include, extend and prepend mixins (directives inherit, include, extend, prepend), resolved through Zeitwerk or else to the nearest file in the require graph that declares them. Every link but a plain constant reference is transitive, like a Dart export: required code is loaded globally and inherited methods land on the inheritor, so a file passes on what it requires or inherits (a gem's lib/<name>.rb requiring its parts is a barrel). In trace such a row is tagged transitive. In a Rails app, or a gem that calls Zeitwerk::Loader.for_gem, a reference to an autoloaded constant also links to the file that defines it, and in Rails the entry points include everything under config/ and db/, controllers, channels, helpers, mailboxes and every ERB and Jbuilder template under app/, whose embedded Ruby is read for constants. Constants named only in strings or symbols (has_many :comments, const_get, constantize, class_name: \"X\") and computed require paths are not seen, so a file reached only that way is reported dead.",
    ],
    ..core::importers()
  }
}
