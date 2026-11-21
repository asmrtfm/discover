use crate::Jvm;
use crate::common::OrderedObject;
use crate::graph::{Graph, decls};
use discover_core::common::{file_arg, out_json};
use serde_json::json;

// ─────────────────────────────────────────────────────────
// definition — a JVM name has no re-export chain to follow:
// it is declared in the file itself, imported (including
// Java's static imports), visible through the file's own
// package, or through a wildcard (`.*`) import. `via`
// reports which, in that order.
// ─────────────────────────────────────────────────────────
pub fn cmd_definition<J: Jvm>(graph: &Graph<J>, args: &[String]) -> Result<u8, String> {
  let (Some(file), Some(symbol)) = (args.first(), args.get(1)) else {
    return Err(format!("usage: {} definition <file> <symbol>", J::NAME));
  };
  let (path, rel) = file_arg(&graph.project.root, file)?;

  if let Some(d) = decls::<J>(&path).iter().find(|d| &d.name == symbol) {
    out_json(&json!({ "symbol": symbol, "kind": d.kind, "file": rel, "line": d.line, "source": "local" })
      .as_object()
      .map(|m| OrderedObject(m.clone(), &["symbol", "kind", "file", "line", "source"]))
      .expect("object"));
    return Ok(0);
  }

  // An imported member (`import a.B.member`, a wildcard over a declaration's
  // members) reports the declaration that holds it.
  for hit in graph.tables().lookup(symbol, &rel) {
    if hit.via == "local" {
      continue;
    }
    let wanted = hit.name.as_deref().unwrap_or(symbol);
    let Some(d) = graph
      .index
      .get(&hit.file)
      .and_then(|e| e.decls.iter().find(|d| d.name == wanted))
    else {
      continue;
    };
    let mut m = serde_json::Map::new();
    m.insert("symbol".into(), json!(symbol));
    m.insert("kind".into(), json!(d.kind));
    m.insert("file".into(), json!(hit.file));
    m.insert("line".into(), json!(d.line));
    m.insert("via".into(), json!(hit.via));
    if hit.member.is_some() {
      m.insert("member_of".into(), json!(d.name));
    }
    out_json(&OrderedObject(m, &["symbol", "kind", "file", "line", "via", "member_of"]));
    return Ok(0);
  }

  out_json(&OrderedObject(
    json!({ "symbol": symbol, "error": "not found in import graph" })
      .as_object()
      .cloned()
      .expect("object"),
    &["symbol", "error"],
  ));
  Ok(1)
}
