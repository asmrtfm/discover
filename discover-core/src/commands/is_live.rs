use crate::common::{file_arg, out};
use crate::graph::{Graph, Lang};

/// Prints LIVE: or DEAD: for the file, exiting 0 when it is live and 1 when it is dead.
pub fn cmd_is_live<L: Lang>(graph: &Graph<L>, args: &[String]) -> Result<u8, String> {
  let file = args.first().ok_or(format!("usage: {} is-live <file>", L::NAME))?;
  let (_, rel) = file_arg(graph.root(), file)?;
  if graph.live()?.contains(&rel) {
    out(&format!("LIVE: {rel}"));
    Ok(0)
  } else {
    out(&format!("DEAD: {rel}"));
    Ok(1)
  }
}
