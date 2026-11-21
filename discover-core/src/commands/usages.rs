//! `usages`: its arguments, its result rows and the helpers every language formats them with.

use crate::common::out_json;
use crate::graph::Lang;
use serde::Serialize;
use std::collections::HashSet;
use std::hash::Hash;

#[derive(Clone, Debug, Serialize)]
pub struct Usage {
  pub file: String,
  pub line: usize,
  pub text: String,
}

/// A source line as reported in results: one leading space removed
/// (jq's `ltrimstr(" ")` in the bash version).
pub fn line_text(src: &str, line: usize) -> String {
  let text = src.lines().nth(line - 1).unwrap_or("");
  text.strip_prefix(' ').unwrap_or(text).to_string()
}

/// Sorted by `key` (file then line), keeping the first row per key.
pub fn sorted_unique<T, K: Ord + Hash>(mut rows: Vec<T>, key: impl Fn(&T) -> K) -> Vec<T> {
  rows.sort_by_key(|r| key(r));
  let mut seen = HashSet::new();
  rows.retain(|r| seen.insert(key(r)));
  rows
}

/// The usual key: one row per file:line.
pub fn file_line(u: &Usage) -> (String, usize) {
  (u.file.clone(), u.line)
}

pub enum UsagesArgs<'a> {
  Pattern(&'a str),
  Assets(&'a str),
}

/// `usages <pattern>` or `usages --assets <asset-path>`.
pub fn parse_usages<'a, L: Lang>(args: &'a [String]) -> Result<UsagesArgs<'a>, String> {
  let mut pattern = "";
  let mut asset = None;
  let mut it = args.iter();
  while let Some(arg) = it.next() {
    match arg.as_str() {
      "--assets" => {
        asset = Some(
          it.next()
            .filter(|a| !a.is_empty())
            .ok_or("--assets requires an asset path argument")?
            .as_str(),
        )
      }
      other => pattern = other,
    }
  }
  if let Some(asset) = asset {
    return Ok(UsagesArgs::Assets(asset));
  }
  if pattern.is_empty() {
    let name = L::NAME;
    return Err(format!(
      "usage: {name} usages <pattern> [--live-only] [-v|--verbose]\n       {name} usages --assets <asset-path>"
    ));
  }
  Ok(UsagesArgs::Pattern(pattern))
}

/// Pattern results on stdout so they can be piped; the count is commentary.
pub fn print_usages(found: &[Usage]) {
  out_json(&found);
  eprintln!("{} usages found", found.len());
}
