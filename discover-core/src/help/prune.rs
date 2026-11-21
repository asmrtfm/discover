use super::Page;

/// `prune`'s generic page.
pub fn prune() -> Page {
  Page {
    summary: "remove intentionally deleted files from the caches",
    synopsis: &["prune FILE..."],
    description: &[
      "Remove each FILE from the caches, so runs with --no-rebuild don't keep reporting intentionally deleted files as stale. A FILE that still exists, or that the caches don't mention, is skipped with a warning.",
      "It can also avoid an automatic rebuild after a deletion, but that is only a side effect and should not be relied on.",
    ],
    options: &[("--strict", "Fail without removing anything when a FILE still exists.")],
  }
}
