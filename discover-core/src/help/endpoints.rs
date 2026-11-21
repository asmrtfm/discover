use super::Page;

/// `endpoints`'s generic page.
pub fn endpoints() -> Page {
  Page {
    summary: "list the API requests the code makes",
    synopsis: &["endpoints [REGEX]..."],
    description: &["Print URL FILE for each API request whose URL, as written in the source, matches any REGEX (unanchored), where FILE is the file the request is made in; with no REGEX, every request."],
    options: &[("-v, --verbose", "Print a line for every file on the request's call stack.")],
  }
}
