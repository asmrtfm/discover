use super::Page;

/// `is-live`'s generic page.
pub fn is_live() -> Page {
  Page {
    summary: "check whether a file is live or dead",
    synopsis: &["is-live FILE"],
    description: &["Check whether FILE is live or dead. If it is live, print LIVE: FILE and exit 0 (true); if it is dead, print DEAD: FILE and exit 1 (false), so it can be used directly in a shell condition."],
    options: &[],
  }
}
