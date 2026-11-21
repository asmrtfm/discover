//! Logic shared by more than one of Ruby's subcommands.

/// Whether text names a constant path (`User`, `Admin::User`, `::User`) rather than a file.
pub fn is_constant(text: &str) -> bool {
  let path = text.strip_prefix("::").unwrap_or(text);
  !path.is_empty() && path.split("::").all(|s| s.starts_with(|c: char| c.is_ascii_uppercase()) && s.chars().all(|c| c.is_alphanumeric() || c == '_'))
}
