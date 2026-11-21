use discover_core::help::{Page, endpoints as core};

/// Dart's `endpoints`: what counts as a request.
pub fn endpoints() -> Page {
  Page {
    description: &[
      "Print URL FILE for each API request whose URL, as written in the source, matches any REGEX (unanchored), where FILE is the file the request is made in; with no REGEX, every request.",
      "Requests are Retrofit endpoint annotations, prefixed with their client's baseUrl; string literals starting with an API root ($rootURL, $postsRoute, $...Root or /api/); and absolute http(s) URLs passed to get, post, put, patch or delete.",
    ],
    ..core::endpoints()
  }
}
