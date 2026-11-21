use super::Page;

/// `config`'s page, the same for every language, since it runs before any language is chosen.
pub fn config() -> Page {
  Page {
    summary: "write a discover.conf for the current directory",
    synopsis: &["config"],
    description: &["Write a commented ./discover.conf for the current directory, naming the detected language and every language's default entry points. An existing discover.conf is left alone. Must be the first argument."],
    options: &[],
  }
}
