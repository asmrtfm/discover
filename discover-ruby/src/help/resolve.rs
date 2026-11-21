use discover_core::help::{Page, resolve as core};

/// Ruby's `resolve`: the rule of each require-style directive, and constants resolved through Zeitwerk.
pub fn resolve() -> Page {
  Page {
    synopsis: &["resolve SPECIFIER [--from FILE] [--require|--require-relative|--load|--autoload]"],
    description: &[
      "Print the repo-relative path a require-style SPECIFIER resolves to, by the rule of the given directive: require and autoload search lib/; require_relative resolves against FILE's directory; load takes the file name as given, from the project root or lib/. Without a directive flag, require_relative when --from is given, else require.",
      "A SPECIFIER that is a constant (Admin::User) resolves to the file Zeitwerk autoloads it from, looked up in FILE's namespaces when given.",
    ],
    options: &[
      ("--from FILE", "Resolve SPECIFIER as FILE sees it."),
      ("--require, --require-relative, --load, --autoload", "Resolve SPECIFIER by that directive's rule."),
    ],
    ..core::resolve()
  }
}
