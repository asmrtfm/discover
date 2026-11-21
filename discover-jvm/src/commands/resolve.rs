use crate::Jvm;
use crate::graph::{Graph, JvmLang};
use discover_core::commands::resolve::parse_resolve;
use discover_core::common::out;
use std::collections::BTreeSet;

// ─────────────────────────────────────────────────────────
// resolve — the file(s) declaring an import path
// (`com.example.Foo`, `com.example.*`), or — with a
// from-file — a simple name as that file sees it. Nothing
// for names the project doesn't declare.
// ─────────────────────────────────────────────────────────
pub fn cmd_resolve<J: Jvm>(graph: &Graph<J>, args: &[String]) -> Result<u8, String> {
  let (uri, from) = parse_resolve::<JvmLang<J>>(args, "<import>")?;
  let mut from = from.to_string();
  if !from.is_empty() {
    from = std::fs::canonicalize(graph.project.to_abs(&from))
      .map(|p| graph.project.rel(&p))
      .unwrap_or(from);
  }
  let files: BTreeSet<String> = graph.tables().lookup(uri, &from).into_iter().map(|h| h.file).collect();
  files.iter().for_each(|f| out(f));
  Ok(0)
}
