use discover_core::help::{Page, usages as core};

/// Dart's `usages`: it searches lib/, leaves out the symbol's own definitions, and finds assets through String constants too.
pub fn usages() -> Page {
  Page {
    description: &["Print every match of the ast-grep PATTERN under lib/ as JSON. The symbol's own definitions are left out."],
    options: &[
      ("--assets PATH", "Print the live references to the asset at PATH as JSON instead: through static String constants that hold it, and as string literals."),
      ("-v, --verbose", "Include the symbol's own definitions."),
    ],
    ..core::usages()
  }
}
