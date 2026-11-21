use crate::graph::Graph;
use crate::ts::{Directive, is_script};
use discover_core::commands::inspect::{EntityOut, parse_inspect};
use discover_core::common::{file_arg, out, out_json};
use serde::Serialize;

#[derive(Serialize)]
struct DirectiveOut {
  uri: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  resolved: Option<String>,
}

#[derive(Serialize)]
struct InspectOut<'a> {
  file: &'a str,
  #[serde(skip_serializing_if = "Option::is_none")]
  imports: Option<Vec<DirectiveOut>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  exports: Option<Vec<DirectiveOut>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  dynamic: Option<Vec<DirectiveOut>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  entities: Option<Vec<EntityOut<'a>>>,
}

/// Prints a module's imports, exports, dynamic imports and top-level entities.
pub fn cmd_inspect(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let parsed = parse_inspect(args, &["--imports", "--exports", "--dynamic"]);
  let file = parsed.file.ok_or(
    "usage: discover-typescript inspect <file> [--imports] [--exports] [--dynamic] [--entities] [--depth names|signatures|full]",
  )?;
  let [imports, exports, dynamic] = parsed.sections[..] else {
    unreachable!("one flag per section")
  };
  let (entities, depth) = (parsed.entities, parsed.depth.as_str());
  let (path, rel) = file_arg(&graph.project.root, file)?;
  if !is_script(&rel) {
    return Err(format!("not a TypeScript/JavaScript module: {file}"));
  }
  let facts = graph.facts(&path);

  // --depth names: plain text, one name per line, no JSON wrapper.
  if entities && depth == "names" {
    facts.entities.iter().for_each(|e| out(&e.name));
    return Ok(0);
  }

  let directive = |d: Directive| {
    facts
      .specs(d)
      .iter()
      .map(|s| DirectiveOut {
        uri: s.text.clone(),
        resolved: graph.project.resolve_uri(&s.text, &rel),
      })
      .collect::<Vec<_>>()
  };
  out_json(&InspectOut {
    file: &rel,
    imports: imports.then(|| directive(Directive::Import)),
    exports: exports.then(|| directive(Directive::Export)),
    dynamic: dynamic.then(|| directive(Directive::Dynamic)),
    entities: entities.then(|| {
      facts
        .entities
        .iter()
        .map(|e| EntityOut::new(e.kind, &e.name, e.line, &e.header, &e.full, depth))
        .collect()
    }),
  });
  Ok(0)
}
