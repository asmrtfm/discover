use crate::common::OrderedObject;
use crate::graph::{Graph, decls};
use crate::{Decl, Jvm};
use discover_core::commands::inspect::{EntityOut, parse_inspect};
use discover_core::common::{file_arg, out, out_json};
use serde_json::{Value, json};
use std::collections::BTreeSet;

// ─────────────────────────────────────────────────────────
// inspect — package and imports from the index (each import
// with the file(s) it resolves to; none for stdlib and
// libraries), entities from the file itself
// ─────────────────────────────────────────────────────────
fn entity_out<'a>(d: &'a Decl, depth: &str) -> EntityOut<'a> {
  EntityOut::new(&d.kind, &d.name, d.line, &d.header, &d.full, depth)
}

pub fn cmd_inspect<J: Jvm>(graph: &Graph<J>, args: &[String]) -> Result<u8, String> {
  let parsed = parse_inspect(args, &["--package", "--imports"]);
  let file = parsed.file.ok_or(format!(
    "usage: {} inspect <file> [--package] [--imports] [--entities] [--depth names|signatures|full]",
    J::NAME
  ))?;
  let [package, imports] = parsed.sections[..] else {
    unreachable!("one flag per section")
  };
  let (entities_on, depth) = (parsed.entities, parsed.depth.as_str());
  let (path, rel) = file_arg(&graph.project.root, file)?;

  // --depth names: plain text, one name per line, no JSON wrapper.
  if entities_on && depth == "names" {
    decls::<J>(&path).iter().for_each(|d| out(&d.name));
    return Ok(0);
  }

  let entry = graph.index.get(&rel);
  let mut result = serde_json::Map::new();
  result.insert("file".into(), json!(rel));
  if package {
    result.insert("package".into(), json!(entry.map(|e| e.pkg.clone())));
  }
  if imports {
    let tables = graph.tables();
    let list: Vec<Value> = entry
      .map(|e| e.imports.as_slice())
      .unwrap_or_default()
      .iter()
      .map(|imp| {
        let spec = if imp.wildcard { format!("{}.*", imp.path) } else { imp.path.clone() };
        let files: BTreeSet<String> = tables.lookup(&spec, "").into_iter().map(|h| h.file).collect();
        let mut row = serde_json::Map::new();
        row.insert("uri".into(), json!(imp.spec));
        match files.len() {
          0 => {}
          1 => {
            row.insert("resolved".into(), json!(files.into_iter().next()));
          }
          _ => {
            row.insert("resolved".into(), json!(files));
          }
        }
        Value::Object(row)
      })
      .collect();
    result.insert("imports".into(), Value::Array(list));
  }
  if entities_on {
    let decls = decls::<J>(&path);
    let list: Vec<EntityOut> = decls.iter().map(|d| entity_out(d, depth)).collect();
    result.insert("entities".into(), serde_json::to_value(list).expect("serializable"));
  }
  out_json(&OrderedObject(result, &["file", "package", "imports", "entities"]));
  Ok(0)
}
