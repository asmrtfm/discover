// ─────────────────────────────────────────────────────────
// trace — tags each importer of a file:
//   active — live importer that uses the target's entities
//   stale  — live importer that doesn't use any entities
//   zombie — dead importer (the dependency exists but the chain is dead)
// The file is dead when no importer is active, then the
// verdict is cross-checked against is-live.
// Several files produce one JSON document each, in argument
// order, which jq reads as a stream.
// ─────────────────────────────────────────────────────────

use crate::cli::{STRICT, VERBOSE};
use crate::common::{collect_files, cwd_prefix, out_json, strip_path};
use crate::graph::{EntityUse, Graph, Lang, Trace};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct TracedRow {
  file: String,
  directive: String,
  uri: String,
  entities_used: Vec<EntityUse>,
  status: &'static str,
  /// Present (true) when the importer passes the target's entities on to
  /// its own importers (it exports, inherits from or mixes in the target);
  /// see [`Lang::is_transitive`].
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  transitive: bool,
}

#[derive(Serialize)]
// Borrows the file path for the moment the output is serialized.
struct TraceOut<'a> {
  // The target path, as given on the command line.
  file: &'a str,
  entities: Vec<String>,
  importers: Vec<TracedRow>,
  dead: bool,
  active_count: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  warning: Option<String>,
}

pub fn cmd_trace<L: Lang>(graph: &Graph<L>, args: &[String]) -> Result<u8, String> {
  // Rejects a call with no file paths.
  if args.is_empty() {
    // Returns the usage message as the error.
    return Err(format!("usage: {} trace [--strict] [-v|--verbose] <files ...>", L::NAME));
  }
  // Names the failure in the warning for a path that isn't a file.
  let msg = "does not exist";
  // Skips, with a warning, any path that isn't a file when strict; otherwise keeps every path unchecked, since trace answers from the index and a deleted file can still have importers.
  let targets = if *STRICT {
    collect_files(true, msg, args)
  } else {
    // Builds the working directory prefix once for all paths.
    let cwd = cwd_prefix();
    // Strips each path to the form the index is keyed by.
    args.iter().map(|arg| strip_path(arg, cwd.as_deref()).to_string()).collect()
  };
  // Loaded once and shared: the reachability memo stays valid across targets.
  let index = graph.importers()?;
  let live = graph.live()?;
  let mut trace = Trace::new(&index, graph.entries());

  // Visits the targets in argument order.
  for file in &targets {
    // Views the argument as a path for the file-based lookups.
    let path = Path::new(file);
    // Skips a file that trace can't follow.
    if !graph.is_source(path) {
      // Warns about a non-source file, in strict mode too, and points to the command for assets.
      eprintln!("warning: {file} is not a source file (trace is for code; for assets use: {} usages --assets <path>)", L::NAME);
      // Moves on to the next target.
      continue;
    }
    // Reads the names of the entities the file declares.
    let mut entities = graph.entity_names(path);
    // Drops the private ones, which no importer can use, unless -v/--verbose asks for every entity.
    if !*VERBOSE {
      entities.retain(|name| !L::is_private(name));
    }
    // Builds a tagged row for each importer stored under this file in the index.
    let mut importers: Vec<TracedRow> = index
      // Looks up this file's importer rows, using the argument as the key.
      .get(file)
      // Treats the rows as a slice.
      .map(Vec::as_slice)
      .unwrap_or_default()
      .iter()
      .map(|row| {
        let status = if !trace.reachable(&row.file) {
          "zombie"
        } else if row.entities_used.is_empty() {
          "stale"
        } else {
          "active"
        };
        TracedRow {
          file: row.file.clone(),
          directive: row.directive.clone(),
          uri: row.uri.clone(),
          entities_used: row.entities_used.clone(),
          status,
          transitive: L::is_transitive(&row.directive),
        }
      })
      .collect();
    importers.sort_by(|a, b| a.file.cmp(&b.file));

    let active_count = importers.iter().filter(|r| r.status == "active").count();
    // An entry point has no importers by definition, yet is live.
    let dead = active_count == 0 && !graph.entries().contains(file);
    // Reads the file's dead verdict from the live list, for the cross-check below.
    let bfs_dead = !live.contains(file);
    let warning = (dead != bfs_dead).then(|| {
      let w = format!("DISAGREEMENT: trace says dead={dead} but is-live says dead={bfs_dead}");
      eprintln!("WARNING: {w}");
      w
    });

    out_json(&TraceOut {
      // Passes the target path without copying it.
      file,
      entities,
      importers,
      dead,
      active_count,
      warning,
    });
  }
  // Exits successfully, including when paths were skipped.
  Ok(0)
}
