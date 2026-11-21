use crate::cli::STRICT;
use crate::common::collect_files;
use crate::graph::{Graph, Lang};

/// Returns the same strings as `collect_files`, or an error if any string failed the constraint.
fn collect_files_strict(constraint: bool, msg: &str, strings: &[String]) -> Result<Vec<String>, String> {
  // Filters the strings, which prints a warning for every failure.
  let files = collect_files(constraint, msg, strings);
  // Detects a failure by comparing how many strings were kept with how many were given.
  if files.len() != strings.len() {
    // Returns the error, with a hint saying how to disable the constraint.
    return Err("--strict: nothing was run\nhint: run without --strict to skip the paths above and continue with the rest".into());
  }
  // Returns the strings, all of which passed.
  Ok(files)
}

/// Lets you keep using --no-rebuild without intentionally deleted files lingering in the caches as stale entries. It can also avoid an automatic rebuild after a deletion, but that is only a side effect and should not be relied on.
pub fn cmd_prune<L: Lang>(graph: &mut Graph<L>, args: &[String]) -> Result<u8, String> {
  // Rejects a call with no file paths.
  if args.is_empty() {
    // Returns the usage message as the error.
    return Err(format!("usage: {} prune [--strict] <files ...>", L::NAME));
  }
  // Names the failure in the warning for a path that is still a file.
  let msg = "still exists";
  // Keeps the paths that are not files, or errors on any that is when strict.
  let targets = if *STRICT { collect_files_strict(false, msg, args)? } else { collect_files(false, msg, args) };
  // Loads the set of paths the caches mention.
  let cached = graph.cached_files();
  // Visits the surviving paths in argument order.
  for file in &targets {
    // Prunes only a path the caches mention.
    if cached.contains(file) {
      // Removes the path from the caches.
      graph.prune(file)?;
    } else {
      // Warns about a path the caches don't mention, in strict mode too.
      eprintln!("warning: {file} is not in the caches");
    }
  }
  // Exits successfully, including when paths were skipped.
  Ok(0)
}
