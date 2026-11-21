use crate::common::is_constant;
use crate::graph::{Graph, RbLang};
use crate::rb::{self, Directive};
use discover_core::commands::definition::{DefinitionArgs, DefinitionOut, out_definition, out_pretty_definition, parse_definition};
use discover_core::common::{file_arg, out_json};
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────
// definition — with a file: its own entities, then the file
// the constant autoloads from, then the require graph below
// the file, nearest first. Without one: every definition of
// the symbol in the project
// ─────────────────────────────────────────────────────────
pub fn cmd_definition(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let DefinitionArgs { symbol, file, verbose, pretty } = parse_definition::<RbLang>(args)?;
  let Some(file) = file else {
    return all_definitions(graph, symbol, verbose, pretty);
  };
  let (path, _) = file_arg(&graph.project.root, file)?;
  let facts = graph.facts(&path);

  // Prints the first entity matching the symbol in a file, if any.
  let found = |target: &PathBuf, via: Option<&[String]>, source: Option<&str>| -> bool {
    let facts = graph.facts(target);
    let Some(e) = facts.entity(symbol).or_else(|| symbol.rsplit("::").next().and_then(|last| facts.entity(last))) else {
      return false;
    };
    let target_rel = graph.project.rel(target);
    let def = DefinitionOut { symbol, kind: e.kind, file: &target_rel, line: e.line, via, source, code: verbose.then_some(e.full.as_str()) };
    out_definition(&def, pretty);
    true
  };

  if facts.entity(symbol).is_some() && found(&path, None, Some("local")) {
    return Ok(0);
  }

  // The constant (or a method's owner, `User#valid?`) as Zeitwerk would load it from this file.
  let owner = symbol.split(['#', '.']).next().unwrap_or(symbol);
  if is_constant(owner) {
    let nesting = facts.constants.iter().find(|c| c.text == owner).map(|c| c.nesting.clone()).unwrap_or_default();
    if let Some(target) = graph.project.resolve_constant(owner, &nesting).and_then(|r| graph.project.existing(&r)) {
      if found(&target, Some(&["(autoload)".to_string()]), None) {
        return Ok(0);
      }
    }
  }

  // Breadth-first down the require-style, superclass and mixin links, so the nearest definition wins; `via` is the chain of requiring or inheriting files.
  let mut visited: HashSet<PathBuf> = HashSet::from([path.clone()]);
  let mut queue: VecDeque<(PathBuf, Vec<String>)> = VecDeque::from([(path, Vec::new())]);
  while let Some((current, via)) = queue.pop_front() {
    for link in graph.links(&current).iter().filter(|l| l.directive != Directive::Constant).cloned() {
      if !visited.insert(link.target.clone()) {
        continue;
      }
      if found(&link.target, Some(&via), None) {
        return Ok(0);
      }
      let mut chain = via.clone();
      chain.push(graph.project.rel(&link.target));
      queue.push_back((link.target, chain));
    }
  }

  out_json(&serde_json::json!({ "symbol": symbol, "error": "not found in require graph" }));
  Ok(1)
}

/// Every declaration of `symbol`, by name or qualified name, in the project's graph files, in file order.
fn all_definitions(graph: &Graph, symbol: &str, verbose: bool, pretty: bool) -> Result<u8, String> {
  let files: Vec<(String, std::sync::Arc<rb::Facts>)> = graph.project.graph_files().iter().map(|f| (graph.project.rel(f), graph.facts(f))).collect();
  let found: Vec<DefinitionOut> = files
    .iter()
    .flat_map(|(rel, facts)| {
      facts.entities.iter().filter(|e| e.name == symbol || e.qualified == symbol).map(move |e| DefinitionOut {
        symbol,
        kind: e.kind,
        file: rel,
        line: e.line,
        via: None,
        source: None,
        code: verbose.then_some(e.full.as_str()),
      })
    })
    .collect();

  if found.is_empty() {
    out_json(&serde_json::json!({ "symbol": symbol, "error": "not found in project" }));
    return Ok(1);
  }
  if pretty {
    found.iter().for_each(out_pretty_definition);
  } else {
    out_json(&found);
  }
  eprintln!("{} definition(s) found", found.len());
  Ok(0)
}
