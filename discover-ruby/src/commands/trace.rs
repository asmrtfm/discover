//! Ruby's `trace [--strict] [--include-upstream] [-v|--verbose] <files ...>`. For each importer of a file, entities_used answers whether the file's entities are used through that importer:
//!   1. in the importer itself;
//!   2. when the importer is a module, in everything downstream of it: the classes and modules that include, extend, prepend or inherit it, through any number of such links;
//!   3. when the importer (or anything downstream of it) is a class, at every callsite of that class in the codebase, ERB templates included.
//! Uses found in another file carry that `file` and the inheritance chain `via` from the importer to it. A row is stale only when none of this finds a use.
//! The entities are those the file declares; --include-upstream adds those of everything it pulls in through its own requires, superclasses and mixins. -v/--verbose also lists each downstream file as a row of its own. The JSON is the shared trace's, with each entities_used entry on one line.

use crate::graph::{Graph, merge_rows, used_entities};
use crate::rb::{Directive, Facts};
use discover_core::cli::{STRICT, VERBOSE};
use discover_core::common::out;
use discover_core::graph::{Importers, Lang, Row, Trace};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;

/// Whether a row's directive passes the target's entities on to the importer's users.
fn transitive(directive: &str) -> bool {
  directive.split(',').any(|d| Directive::named(d).is_some_and(Directive::is_transitive))
}

/// One of the file's entities used through an importer.
#[derive(Clone, Serialize)]
struct Use {
  name: String,
  /// The file the uses are in, when it isn't the importer.
  #[serde(skip_serializing_if = "Option::is_none")]
  file: Option<String>,
  lines: Vec<usize>,
  /// The inheritance chain from the importer to `file`: the importer, then each class or module that includes, extends, prepends or inherits the one before it.
  #[serde(skip_serializing_if = "Vec::is_empty")]
  via: Vec<String>,
}

#[derive(Serialize)]
struct TracedRow {
  file: String,
  directive: String,
  uri: String,
  entities_used: Vec<Use>,
  status: &'static str,
  /// Present (true) when the importer passes the target's entities on to its own users.
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  transitive: bool,
  /// For a row reached by following: the files between it and the target, nearest the target first.
  #[serde(skip_serializing_if = "Vec::is_empty")]
  via: Vec<String>,
}

#[derive(Serialize)]
struct TraceOut<'a> {
  file: &'a str,
  entities: Vec<String>,
  importers: Vec<TracedRow>,
  /// `"maybe"` when no importer is active, else `false`: Ruby's use detection is heuristic, so a dead verdict is only a guess.
  dead: serde_json::Value,
  active_count: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  warning: Option<String>,
}

/// What gains the target through a row, when the row's file inherits it: `class` or `module`, the definition around the superclass or mixin statement (a top-level include counts as a module). None when the row is no ancestor link.
fn inheritor_kind(facts: &Facts, row: &Row) -> Option<&'static str> {
  let directives: Vec<&str> = row.directive.split(',').collect();
  let uris: Vec<&str> = row.uri.split(", ").collect();
  facts
    .ancestors
    .iter()
    .find(|a| directives.contains(&a.directive.as_str()) && uris.contains(&a.text.as_str()))
    .map(|a| if a.owner == "class" { "class" } else { "module" })
}

/// Everything the target's `names` are used through one importer row, per the module's three rules; one entry per name and file, at its shortest chain.
fn row_uses(graph: &Graph, index: &Importers, row: &Row, names: &[String]) -> Vec<Use> {
  let facts = |rel: &str| -> Arc<Facts> { graph.facts(&graph.project.root.join(rel)) };
  // The uses in one file, each name with its lines there.
  let uses_in = |rel: &str, file: Option<&str>, via: &[String]| -> Vec<Use> {
    used_entities(&facts(rel), names)
      .into_iter()
      .map(|u| Use { name: u.name, file: file.map(str::to_string), lines: u.lines, via: via.to_vec() })
      .collect()
  };

  // 1. The importer itself.
  let mut found = uses_in(&row.file, None, &[]);
  // An importer that doesn't inherit the target passes nothing on.
  let Some(kind) = inheritor_kind(&facts(&row.file), row) else { return found };

  // 2 and 3, breadth-first so each file is reached by its shortest chain: inheritors of every module and class down the chain, and callsites of every class.
  let mut visited: HashSet<String> = HashSet::from([row.file.clone()]);
  let mut queue: VecDeque<(String, &'static str, Vec<String>)> = VecDeque::from([(row.file.clone(), kind, vec![row.file.clone()])]);
  while let Some((current, kind, via)) = queue.pop_front() {
    for user in index.get(&current).into_iter().flatten() {
      match inheritor_kind(&facts(&user.file), user) {
        Some(user_kind) => {
          if visited.insert(user.file.clone()) {
            found.extend(uses_in(&user.file, Some(&user.file), &via));
            let mut chain = via.clone();
            chain.push(user.file.clone());
            queue.push_back((user.file.clone(), user_kind, chain));
          }
        }
        // A file that references a class (instantiates it, calls its methods) is a callsite; for a module it uses only the module itself.
        None if kind == "class" && !visited.contains(&user.file) => found.extend(uses_in(&user.file, Some(&user.file), &via)),
        None => {}
      }
    }
  }

  // One entry per name and file: the first chain seen, with every line.
  let mut merged: BTreeMap<(String, Option<String>), Use> = BTreeMap::new();
  for u in found {
    match merged.get_mut(&(u.name.clone(), u.file.clone())) {
      Some(kept) => {
        kept.lines.extend(u.lines);
        kept.lines.sort_unstable();
        kept.lines.dedup();
      }
      None => {
        merged.insert((u.name.clone(), u.file.clone()), u);
      }
    }
  }
  merged.into_values().collect()
}

/// One JSON document per file, in argument order.
pub fn cmd_trace(graph: &Graph, args: &[String]) -> Result<u8, String> {
  // `--include-upstream`: also trace the entities the file pulls in through its own transitive links.
  let upstream = args.iter().any(|a| a == "--include-upstream");
  // Accepts `./x` and repo-relative paths, like the shared trace; every argument but --include-upstream is a file.
  let mut files: Vec<&str> = args.iter().filter(|a| *a != "--include-upstream").map(|f| f.strip_prefix("./").unwrap_or(f)).collect();
  if files.is_empty() {
    return Err("usage: discover --lang=ruby trace [--strict] [--include-upstream] [-v|--verbose] <files ...>".into());
  }
  // Skips, with a warning, any path that isn't a file when strict; otherwise keeps every path unchecked, since trace answers from the index and a deleted file can still have importers.
  if *STRICT {
    files.retain(|file| {
      let exists = Path::new(file).is_file();
      if !exists {
        eprintln!("warning: {file} does not exist");
      }
      exists
    });
  }
  let index = graph.importers()?;
  let live: HashSet<String> = graph.live()?.into_iter().collect();
  let mut reach = Trace::new(&index, graph.entries());

  for file in files {
    if !graph.is_source(Path::new(file)) {
      eprintln!("warning: {file} is not a source file (trace is for code; for assets use: discover --lang=ruby usages --assets <path>)");
      continue;
    }
    let target = graph.project.root.join(file);
    let entities = graph.entity_names(&target);
    // The names whose uses count: the file's own, or with --include-upstream everything it makes available.
    let names: Vec<String> = if upstream { graph.namespace(&target).iter().cloned().collect() } else { entities.clone() };

    // The target's importers, then with -v/--verbose every file reached through their transitive links, breadth-first, each with the chain that leads to it.
    let mut reached: Vec<(Row, Vec<String>)> = Vec::new();
    let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::from([file.to_string()]);
    for row in index.get(file).into_iter().flatten() {
      if *VERBOSE && transitive(&row.directive) && visited.insert(row.file.clone()) {
        queue.push_back((row.file.clone(), vec![row.file.clone()]));
      }
      reached.push((row.clone(), Vec::new()));
    }
    while let Some((through, via)) = queue.pop_front() {
      for row in index.get(&through).into_iter().flatten() {
        if transitive(&row.directive) && visited.insert(row.file.clone()) {
          let mut chain = via.clone();
          chain.push(row.file.clone());
          queue.push_back((row.file.clone(), chain));
        }
        reached.push((row.clone(), via.clone()));
      }
    }

    // One row per file: the first (shortest) chain, with the directives and paths of every way it was reached.
    let mut by_file: Vec<(Row, Vec<String>)> = Vec::new();
    for (row, via) in reached {
      match by_file.iter_mut().find(|(kept, _)| kept.file == row.file) {
        Some((kept, _)) => *kept = merge_rows(vec![kept.clone(), row]).remove(0),
        None => by_file.push((row, via)),
      }
    }
    let mut importers: Vec<TracedRow> = by_file
      .into_iter()
      .map(|(row, via)| {
        let entities_used = row_uses(graph, &index, &row, &names);
        TracedRow {
          // Ruby's reachability and use detection are heuristic, so the negative verdicts are only guesses.
          status: if !reach.reachable(&row.file) { "maybe zombie" } else if entities_used.is_empty() { "maybe stale" } else { "active" },
          transitive: transitive(&row.directive),
          entities_used,
          file: row.file,
          directive: row.directive,
          uri: row.uri,
          via,
        }
      })
      .collect();
    importers.sort_by(|a, b| a.via.len().cmp(&b.via.len()).then_with(|| a.file.cmp(&b.file)));

    let active_count = importers.iter().filter(|r| r.status == "active").count();
    // An entry point has no importers by definition, yet is live.
    let dead = active_count == 0 && !graph.entries().contains(file);
    let bfs_dead = !live.contains(file);
    let warning = (dead != bfs_dead).then(|| {
      let w = format!("DISAGREEMENT: trace says dead={dead} but is-live says dead={bfs_dead}");
      eprintln!("WARNING: {w}");
      w
    });
    // Printed as "maybe" rather than true; the bool above still decides the warning.
    let dead = if dead { serde_json::Value::from("maybe") } else { serde_json::Value::from(false) };
    let doc = serde_json::to_string_pretty(&TraceOut { file, entities, importers, dead, active_count, warning }).map_err(|e| e.to_string())?;
    out(&one_entry_per_line(&doc));
  }
  Ok(0)
}

/// Rewrites jq-style pretty JSON so each entry of an `entities_used` array sits on one line (`{ "name": "as_role", "lines": [12, 30] }`), keeping field order; an entry's `via`, its last field, follows on its own lines, one file per line. Works on whole lines, so string contents are never split.
fn one_entry_per_line(pretty: &str) -> String {
  let mut out: Vec<String> = Vec::new();
  let mut lines = pretty.lines();
  while let Some(line) = lines.next() {
    out.push(line.to_string());
    if !line.trim_end().ends_with("\"entities_used\": [") {
      continue;
    }
    let indent = line.len() - line.trim_start().len();
    let mut entry: Vec<&str> = Vec::new();
    for inner in lines.by_ref() {
      let depth = inner.len() - inner.trim_start().len();
      // The array's closing bracket.
      if depth == indent && inner.trim_start().starts_with(']') {
        out.push(inner.to_string());
        break;
      }
      entry.push(inner.trim());
      // An entry's closing brace.
      if depth == indent + 2 && inner.trim_start().starts_with('}') {
        let pad = |n: usize| " ".repeat(indent + n);
        match entry.iter().position(|p| *p == "\"via\": [") {
          // Everything before via on the entry's line, then via one file per line, closed together with the entry.
          Some(at) => {
            let close = entry.len() - 1;
            out.push(format!("{}{}", pad(2), join_line(&entry[..at])));
            out.push(format!("{}{}", pad(4), entry[at]));
            entry[at + 1..close - 1].iter().for_each(|file| out.push(format!("{}{file}", pad(6))));
            out.push(format!("{}{} {}", pad(4), entry[close - 1], entry[close]));
          }
          None => out.push(format!("{}{}", pad(2), join_line(&entry))),
        }
        entry.clear();
      }
    }
  }
  out.join("\n")
}

/// Joins the trimmed lines of one JSON value with single spaces, none just inside square brackets.
fn join_line(parts: &[&str]) -> String {
  let mut text = String::new();
  for part in parts {
    if !text.is_empty() && !text.ends_with('[') && !part.starts_with(']') {
      text.push(' ');
    }
    text.push_str(part);
  }
  text
}
