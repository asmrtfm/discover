//! `usages`: structural search over the sources with an ast-grep pattern, and the `--assets` lookup of what names an asset file.

use crate::graph::{Graph, RbLang};
use crate::rb::{self, is_ruby};
use ast_grep_core::Pattern;
use ast_grep_language::SupportLang;
use discover_core::cli::LIVE_ONLY;
use discover_core::commands::usages::{Usage, UsagesArgs, file_line, line_text, parse_usages, print_usages, sorted_unique};
use discover_core::par::par_map;
use std::collections::HashSet;
use std::fs;

/// `usages <pattern>`: the pattern runs over every source file, or with --live-only over the live ones. A pattern that doesn't parse as Ruby finds nothing.
pub fn pattern_usages(graph: &Graph, pattern: &str) -> Result<Vec<Usage>, String> {
  let pattern = Pattern::try_new(pattern, SupportLang::Ruby).map_err(|e| format!("pattern does not parse as Ruby: {e}"))?;
  let live: Option<HashSet<String>> = if *LIVE_ONLY { Some(graph.live()?.into_iter().collect()) } else { None };
  let files = graph.project.relevant_files();
  let hits: Vec<Vec<Usage>> = par_map(&files, |path| {
    let rel = graph.project.rel(path);
    if live.as_ref().is_some_and(|l| !l.contains(&rel)) {
      return Vec::new();
    }
    let Ok(src) = fs::read_to_string(path) else { return Vec::new() };
    let root = rb::parse(&src);
    root
      .root()
      .find_all(&pattern)
      .map(|m| {
        let line = m.get_node().start_pos().line() + 1;
        Usage { file: rel.clone(), line, text: line_text(&src, line) }
      })
      .collect()
  });
  Ok(sorted_unique(hits.into_iter().flatten().collect(), file_line))
}

/// The string literals that name an asset: its repo path; for a file under public/, the URL it is served at (`/robots.txt`); and for a file in an asset pipeline directory (`app/assets/images/logo.png`), its logical path (`logo.png`), which `image_tag` and `asset_path` take.
pub fn asset_literals(asset_rel: &str) -> Vec<String> {
  let mut literals = vec![asset_rel.to_string()];
  if let Some(served) = asset_rel.strip_prefix("public/") {
    literals.push(format!("/{served}"));
  }
  for prefix in ["app/assets/", "lib/assets/", "vendor/assets/"] {
    // The directory right below assets/ (images, stylesheets, ...) isn't part of the logical path.
    if let Some((_, logical)) = asset_rel.strip_prefix(prefix).and_then(|rest| rest.split_once('/')) {
      literals.push(logical.to_string());
    }
  }
  literals
}

/// `usages --assets <path>`: the lines of live Ruby files, ERB templates included, with a string literal that names the asset.
pub fn asset_usages(graph: &Graph, asset: &str) -> Result<Vec<Usage>, String> {
  let asset_rel = fs::canonicalize(graph.project.to_abs(asset))
    .ok()
    .filter(|p| p.is_file())
    .map(|p| graph.project.rel(&p))
    .ok_or_else(|| format!("asset not found: {asset}"))?;
  let literals = asset_literals(&asset_rel);
  let live: Vec<String> = graph.live()?.into_iter().filter(|r| is_ruby(r) || graph.project.entry_set.contains(r)).collect();
  let hits: Vec<Vec<Usage>> = par_map(&live, |rel| {
    let path = graph.project.root.join(rel);
    let Ok(src) = fs::read_to_string(&path) else { return Vec::new() };
    let root = rb::parse_file(&path, &src);
    literal_lines(&root, &literals).into_iter().map(|line| Usage { file: rel.clone(), line, text: line_text(&src, line) }).collect()
  });
  Ok(sorted_unique(hits.into_iter().flatten().collect(), file_line))
}

/// Prints the matches of a pattern, or with `--assets` the lines that name an asset.
pub fn cmd_usages(graph: &Graph, args: &[String]) -> Result<u8, String> {
  match parse_usages::<RbLang>(args)? {
    UsagesArgs::Assets(asset) => print_usages(&asset_usages(graph, asset)?),
    UsagesArgs::Pattern(pattern) => print_usages(&pattern_usages(graph, pattern)?),
  }
  Ok(0)
}

/// Lines (1-based) of string literals exactly equal to one of `literals`.
pub fn literal_lines(root: &rb::RbRoot, literals: &[String]) -> Vec<usize> {
  root
    .root()
    .dfs()
    .filter(|n| n.kind() == "string_content")
    .filter(|n| literals.iter().any(|l| n.text() == l.as_str()))
    .map(|n| rb::line_of(&n))
    .collect()
}

