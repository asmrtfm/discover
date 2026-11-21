use discover_core::help::{Page, trace as core};

/// Dart's `trace`: which entities are private.
pub fn trace() -> Page {
  Page {
    options: &[("-v, --verbose", "Include the entities private to FILE (names starting with _).")],
    ..core::trace()
  }
}
