use crate::graph::Graph;
use discover_core::commands::inspect::{EntityOut, parse_inspect};
use discover_core::common::{file_arg, out, out_json};
use serde::Serialize;

#[derive(Serialize)]
struct RequireOut<'a> {
  directive: &'static str,
  uri: &'a str,
  line: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  resolved: Option<String>,
}

#[derive(Serialize)]
struct ConstantOut<'a> {
  constant: &'a str,
  line: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  resolved: Option<String>,
}

#[derive(Serialize)]
struct AncestorOut<'a> {
  directive: &'static str,
  constant: &'a str,
  line: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  resolved: Option<String>,
}

#[derive(Serialize)]
struct InspectOut<'a> {
  file: &'a str,
  #[serde(skip_serializing_if = "Option::is_none")]
  requires: Option<Vec<RequireOut<'a>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  ancestors: Option<Vec<AncestorOut<'a>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  constants: Option<Vec<ConstantOut<'a>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  entities: Option<Vec<EntityOut<'a>>>,
}

/// Prints a file's requires, ancestors, referenced constants and entities.
pub fn cmd_inspect(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let parsed = parse_inspect(args, &["--requires", "--ancestors", "--constants"]);
  let file = parsed.file.ok_or("usage: discover --lang=ruby inspect <file> [--requires] [--ancestors] [--constants] [--entities] [--depth names|signatures|full]")?;
  let [requires, ancestors, constants] = parsed.sections[..] else { unreachable!("one flag per section") };
  // Without Zeitwerk no constant resolves, so the list is only shown when asked for.
  let constants = constants && (graph.project.autoloads() || args.iter().any(|a| a == "--constants"));
  let (entities, depth) = (parsed.entities, parsed.depth.as_str());
  let (path, rel) = file_arg(&graph.project.root, file)?;
  let facts = graph.facts(&path);

  // --depth names: plain text, one name per line, no JSON wrapper.
  if entities && depth == "names" {
    facts.entities.iter().for_each(|e| out(&e.name));
    return Ok(0);
  }

  out_json(&InspectOut {
    file: &rel,
    requires: requires.then(|| {
      facts
        .specs
        .iter()
        .map(|s| RequireOut { directive: s.directive.as_str(), uri: &s.text, line: s.line, resolved: graph.project.resolve_spec(s, &rel) })
        .collect()
    }),
    ancestors: ancestors.then(|| {
      // Each ancestor resolved to the file its link points at.
      let links = graph.links(&path);
      facts
        .ancestors
        .iter()
        .map(|a| AncestorOut {
          directive: a.directive.as_str(),
          constant: &a.text,
          line: a.line,
          resolved: links.iter().find(|l| l.directive == a.directive && l.uri == a.text).map(|l| graph.project.rel(&l.target)),
        })
        .collect()
    }),
    constants: constants.then(|| {
      facts
        .constants
        .iter()
        .map(|c| ConstantOut { constant: &c.text, line: c.line, resolved: graph.project.resolve_constant(&c.text, &c.nesting).filter(|r| *r != rel) })
        .collect()
    }),
    entities: entities.then(|| facts.entities.iter().map(|e| EntityOut::new(e.kind, &e.name, e.line, &e.header, &e.full, depth)).collect()),
  });
  Ok(0)
}
