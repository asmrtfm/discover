use crate::graph::{DartLang, Graph};
use discover_core::commands::resolve::parse_resolve;
use discover_core::common::out;

/// Prints the repo-relative path a directive URI resolves to, relative to `--from` when given.
pub fn cmd_resolve(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let (uri, from) = parse_resolve::<DartLang>(args, "<uri>")?;
  if let Some(resolved) = graph.project.resolve_uri(uri, from) {
    out(&resolved);
  }
  Ok(0)
}
