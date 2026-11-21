use crate::graph::{Graph, TsLang};
use discover_core::commands::resolve::parse_resolve;
use discover_core::common::out;

/// Prints the repo-relative path an import specifier resolves to, relative to `--from` when given.
pub fn cmd_resolve(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let (uri, from) = parse_resolve::<TsLang>(args, "<specifier>")?;
  if let Some(resolved) = graph.project.resolve_uri(uri, from) {
    out(&resolved);
  }
  Ok(0)
}
