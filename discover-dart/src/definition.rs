//! `definition`: where a symbol is defined. With a file, among the file's own definitions, then its imports (following re-exports), then its parts; without one, every definition of the symbol in the project.

use crate::common;
use crate::dart::{Definition, Directive};
use crate::graph::{DartLang, Graph};
use discover_core::cli::{
  DefinitionArgs, DefinitionOut, file_arg, out_definition, out_json, out_pretty_definition, parse_definition,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The first definition (by line) with this name.
fn first<'d>(definitions: &'d [Definition], name: &str) -> Option<&'d Definition> {
  definitions.iter().find(|d| d.name == name)
}

// ─────────────────────────────────────────────────────────
// definition — with a file: local entities first, then each
// import, following re-exports; then part files. Without one:
// every definition of the symbol in the project
// ─────────────────────────────────────────────────────────
pub fn cmd_definition(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let DefinitionArgs { symbol, file, verbose, pretty } = parse_definition::<DartLang>(args)?;
  let Some(file) = file else {
    return all_definitions(graph, symbol, verbose, pretty);
  };
  let (path, rel) = file_arg(&graph.project.root, file)?;
  let facts = graph.facts(&path);

  if let Some(e) = first(&facts.definitions, symbol) {
    let def = DefinitionOut {
      symbol,
      kind: e.kind,
      file: &rel,
      line: e.line,
      via: None,
      source: Some("local"),
      code: verbose.then_some(e.full.as_str()),
    };
    out_definition(&def, pretty);
    return Ok(0);
  }

  let mut visited = HashSet::new();
  let searches = [(Directive::Import, Vec::new()), (Directive::Part, vec!["(part)".to_string()])];
  for (directive, via) in searches {
    for uri in facts.uris(directive) {
      let Some(resolved) = graph.project.resolve_uri(uri, &rel) else {
        continue;
      };
      let path = graph.project.root.join(resolved);
      if search_definition(graph, symbol, &path, &via, (verbose, pretty), &mut visited) {
        return Ok(0);
      }
    }
  }

  out_json(&serde_json::json!({ "symbol": symbol, "error": "not found in import graph" }));
  Ok(1)
}

/// Every declaration of `symbol` in the project's source files and their
/// generated extensions (`*.g.dart`, `*.freezed.dart`), in file order.
fn all_definitions(graph: &Graph, symbol: &str, verbose: bool, pretty: bool) -> Result<u8, String> {
  let files = common::project_facts(graph);

  let found: Vec<DefinitionOut> = files
    .iter()
    .flat_map(|(rel, facts)| {
      // Leaves out constructors, which are part of their class's definition rather than definitions of their own.
      facts.definitions.iter().filter(|e| e.name == symbol && e.kind != "constructor").map(move |e| DefinitionOut {
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

/// Looks for `symbol` among a file's entities, then down its re-exports.
/// `via` is the chain of re-exporting files walked so far; `output` is
/// `(verbose, pretty)`.
fn search_definition(
  graph: &Graph,
  symbol: &str,
  path: &Path,
  via: &[String],
  output: (bool, bool),
  visited: &mut HashSet<PathBuf>,
) -> bool {
  let (verbose, pretty) = output;
  let Ok(path) = std::fs::canonicalize(path) else {
    return false;
  };
  if !path.is_file() || !visited.insert(path.clone()) {
    return false;
  }
  let rel = graph.project.rel(&path);
  let facts = graph.facts(&path);
  if let Some(e) = first(&facts.definitions, symbol) {
    let def = DefinitionOut {
      symbol,
      kind: e.kind,
      file: &rel,
      line: e.line,
      via: Some(via),
      source: None,
      code: verbose.then_some(e.full.as_str()),
    };
    out_definition(&def, pretty);
    return true;
  }
  for uri in facts.uris(Directive::Export) {
    let Some(target) = graph.resolve(uri, &rel) else {
      continue;
    };
    let mut chain = via.to_vec();
    chain.push(rel.clone());
    if search_definition(graph, symbol, &target, &chain, output, visited) {
      return true;
    }
  }
  false
}
