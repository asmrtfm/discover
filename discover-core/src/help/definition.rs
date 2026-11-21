use super::Page;

/// `definition`'s generic page.
pub fn definition() -> Page {
  Page {
    summary: "print where a symbol is defined",
    synopsis: &["definition SYMBOL [FILE] [-p|--pretty]"],
    description: &["Print where SYMBOL, as used in FILE, is defined, as JSON. Without FILE, print every definition of SYMBOL in the project."],
    options: &[
      ("-p, --pretty", "Print a FILE:LINE KIND SYMBOL header followed by the code as written, instead of JSON."),
      ("-v, --verbose", "Add each definition's code."),
    ],
  }
}
