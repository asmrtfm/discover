use crate::graph::Graph;
use crate::ts::{Directive, is_script};
use discover_core::commands::definition::DefinitionOut;
use discover_core::common::{file_arg, out_json};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────
// definition — local entities first; then the import that
// binds the symbol; then every import by name; then dynamic
// imports. Re-exports are followed through `export … from`,
// `export * from` and import-then-export.
// ─────────────────────────────────────────────────────────
pub fn cmd_definition(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let (Some(file), Some(symbol)) = (args.first(), args.get(1)) else {
    return Err("usage: discover-typescript definition <file> <symbol>".into());
  };
  let (path, rel) = file_arg(&graph.project.root, file)?;
  let facts = graph.facts(&path);

  if let Some(e) = facts.entity(symbol) {
    out_json(&DefinitionOut {
      symbol,
      kind: e.kind,
      file: &rel,
      line: e.line,
      via: None,
      source: Some("local"),
      code: None,
    });
    return Ok(0);
  }

  // Searched by (file, name): a barrel can be searched once per name it re-exports.
  let mut visited = HashSet::new();
  let mut search = |target: &str, name: &str, via: &[String]| {
    search_definition(graph, symbol, &graph.project.root.join(target), name, via, &mut visited)
  };

  // The symbol is an import binding in this file: follow exactly that import.
  for b in facts.bindings.iter().filter(|b| {
    matches!(b.id, "import-spec" | "import-default") && b.local.as_deref() == Some(symbol.as_str())
  }) {
    let (Some(src), Some(name)) = (&b.src, &b.name) else { continue };
    if let Some(resolved) = graph.project.resolve_uri(src, &rel) {
      if search(&resolved, name, &[]) {
        return Ok(0);
      }
    }
  }

  // Otherwise search each import by name (covers namespace access, `ns.Foo`),
  // then dynamically loaded modules.
  let searches = [(Directive::Import, Vec::new()), (Directive::Dynamic, vec!["(dynamic)".to_string()])];
  for (directive, via) in searches {
    for spec in facts.specs(directive) {
      let Some(resolved) = graph.project.resolve_uri(&spec.text, &rel) else {
        continue;
      };
      if search(&resolved, symbol, &via) {
        return Ok(0);
      }
    }
  }

  out_json(&serde_json::json!({ "symbol": symbol, "error": "not found in import graph" }));
  Ok(1)
}

/// Looks for `name` (what the module itself calls the symbol) in a file's
/// entities, then through its re-exports. `via` is the chain of
/// re-exporting files walked so far.
fn search_definition(
  graph: &Graph,
  symbol: &str,
  path: &Path,
  name: &str,
  via: &[String],
  visited: &mut HashSet<(PathBuf, String)>,
) -> bool {
  let Ok(path) = std::fs::canonicalize(path) else {
    return false;
  };
  if !path.is_file() || !is_script(&path.to_string_lossy()) {
    return false;
  }
  if !visited.insert((path.clone(), name.to_string())) {
    return false;
  }
  let rel = graph.project.rel(&path);
  let facts = graph.facts(&path);

  // A default import names whatever this module's default export declares.
  let name = if name == "default" {
    match facts.default_export() {
      Some(n) => n.to_string(),
      None => return false,
    }
  } else {
    name.to_string()
  };

  if let Some(e) = facts.entity(&name) {
    out_json(&DefinitionOut {
      symbol,
      kind: e.kind,
      file: &rel,
      line: e.line,
      via: Some(via),
      source: None,
      code: None,
    });
    return true;
  }

  // Follow re-exports: `export { X } from`, `export * from` (which never
  // carries a default export), and `import { X } …; export { X }`.
  let mut next: Vec<(String, String)> = Vec::new();
  for b in &facts.bindings {
    let Some(src) = &b.src else { continue };
    match b.id {
      "export-star" if name != "default" => next.push((src.clone(), name.clone())),
      "export-spec" | "import-spec" | "import-default" if b.local.as_deref() == Some(name.as_str()) => {
        if let Some(orig) = &b.name {
          next.push((src.clone(), orig.clone()));
        }
      }
      _ => {}
    }
  }
  for (src, orig) in next {
    let Some(target) = graph.resolve(&src, &rel) else { continue };
    let mut chain = via.to_vec();
    chain.push(rel.clone());
    if search_definition(graph, symbol, &target, &orig, &chain, visited) {
      return true;
    }
  }
  false
}
