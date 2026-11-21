use crate::cli::STRICT;
use crate::common::{collect_files, cwd_prefix, out, strip_path};
use crate::graph::{Graph, Lang, Row};

/// One importer per line. With several files, each line is `<target>:<importer>`, like grep with several files, so every importer stays attributed to its target.
pub fn cmd_importers<L: Lang>(graph: &Graph<L>, args: &[String]) -> Result<u8, String> {
  // Rejects a call with no file paths.
  if args.is_empty() {
    // Returns the usage message as the error.
    return Err(format!("usage: {} importers [--strict] <files ...>", L::NAME));
  }
  // Names the failure in the warning for a path that isn't a file.
  let msg = "does not exist";
  // Skips, with a warning, any path that isn't a file when strict; otherwise keeps every path unchecked, since importers answers from the index and a deleted file can still have importers.
  let targets = if *STRICT {
    collect_files(true, msg, args)
  } else {
    // Builds the working directory prefix once for all paths.
    let cwd = cwd_prefix();
    // Strips each path to the form the index is keyed by.
    args.iter().map(|arg| strip_path(arg, cwd.as_deref()).to_string()).collect()
  };
  // Loads the importers index once for all targets.
  let index = graph.importers()?;
  // Visits the targets in argument order.
  for file in &targets {
    // Gets this target's importer rows, or none when nothing imports it.
    let mut rows: Vec<&Row> = index.get(file).map(|rows| rows.iter().collect()).unwrap_or_default();
    // Sorts the rows by importer path.
    rows.sort_by(|a, b| a.file.cmp(&b.file));
    // Prints one line per importer.
    for row in rows {
      // Uses the argument count, so a skipped path can't change the output format.
      if args.len() > 1 {
        // Prefixes the target so each importer stays tied to it.
        out(&format!("{file}:{}", row.file));
      } else {
        // Prints the bare importer path for a single argument.
        out(&row.file);
      }
    }
  }
  // Exits successfully, including when paths were skipped.
  Ok(0)
}
