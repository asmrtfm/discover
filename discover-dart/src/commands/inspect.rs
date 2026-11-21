use crate::dart::Directive;
use crate::graph::Graph;
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
  parts: Option<Vec<DirectiveOut>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  entities: Option<Vec<EntityOut<'a>>>,
}

/// Prints a file's imports, exports, parts and top-level entities.
pub fn cmd_inspect(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let parsed = parse_inspect(args, &["--imports", "--exports", "--parts"]);
  let file = parsed.file.ok_or(
    "usage: discover-dart inspect <file> [--imports] [--exports] [--parts] [--entities] [--depth names|signatures|full]",
  )?;
  let [imports, exports, parts] = parsed.sections[..] else {
    unreachable!("one flag per section")
  };
  let (entities, depth) = (parsed.entities, parsed.depth.as_str());
  let (path, rel) = file_arg(&graph.project.root, file)?;
  let facts = graph.facts(&path);

  // --depth names: plain text, one name per line, no JSON wrapper.
  if entities && depth == "names" {
    facts.entities.iter().for_each(|e| out(&e.name));
    return Ok(0);
  }

  let directive = |d: Directive| {
    facts
      .uris(d)
      .iter()
      .map(|uri| DirectiveOut {
        uri: uri.clone(),
        resolved: graph.project.resolve_uri(uri, &rel),
      })
      .collect::<Vec<_>>()
  };
  out_json(&InspectOut {
    file: &rel,
    imports: imports.then(|| directive(Directive::Import)),
    exports: exports.then(|| directive(Directive::Export)),
    parts: parts.then(|| directive(Directive::Part)),
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
