use crate::project::{relative, to_abs};
use serde::Serialize;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Writes a line to stdout; a closed pipe (`| head`) ends the run quietly.
pub fn out(line: &str) {
  let mut stdout = io::stdout().lock();
  if writeln!(stdout, "{line}").is_err() {
    std::process::exit(0);
  }
}

/// Pretty JSON, indented like jq's default output.
pub fn out_json<T: Serialize>(value: &T) {
  out(&serde_json::to_string_pretty(value).expect("serializable"));
}

/// Builds the current working directory prefix that `strip_path` removes, or None when it can't be read.
pub(crate) fn cwd_prefix() -> Option<String> {
  // Appends the separator so only whole leading directories are stripped.
  std::env::current_dir().ok().map(|dir| format!("{}/", dir.display()))
}

/// Removes a leading `./`, or else a leading working directory, leaving any other string as it is.
pub(crate) fn strip_path<'a>(maybe_file: &'a str, cwd: Option<&str>) -> &'a str {
  maybe_file
    // Tries the `./` prefix first.
    .strip_prefix("./")
    // Tries the working directory prefix when `./` wasn't there.
    .or_else(|| maybe_file.strip_prefix(cwd?))
    // Falls back to the string as given.
    .unwrap_or(maybe_file)
}

/// Returns the strings whose existence as a file equals `constraint`, and prints a warning containing `msg` for each of the others.
pub(crate) fn collect_files(constraint: bool, msg: &str, strings: &[String]) -> Vec<String> {
  // Holds the strings that satisfy the constraint.
  let mut files = Vec::with_capacity(strings.len());
  // Builds the current working directory prefix, or None when it can't be read.
  let cwd = cwd_prefix();
  // Visits every string, in argument order.
  for maybe_file in strings {
    // Removes a leading `./`, or else a leading working directory.
    let stripped = strip_path(maybe_file, cwd.as_deref());
    // Compares whether the path is a file with the constraint.
    if Path::new(stripped).is_file() == constraint {
      // Keeps an owned copy of the stripped form of a string that satisfies the constraint.
      files.push(stripped.to_string());
    } else {
      // Warns about a string that doesn't satisfy the constraint.
      eprintln!("warning: {maybe_file} {msg}");
    }
  }
  // Returns the strings that were kept.
  files
}

/// An existing file argument as (canonical path, repo-relative path).
pub fn file_arg(root: &Path, file: &str) -> Result<(PathBuf, String), String> {
  let abs = to_abs(root, file);
  if !abs.is_file() {
    return Err(format!("file not found: {file}"));
  }
  let canonical = std::fs::canonicalize(&abs).map_err(|e| format!("{file}: {e}"))?;
  let rel = relative(root, &canonical);
  Ok((canonical, rel))
}
