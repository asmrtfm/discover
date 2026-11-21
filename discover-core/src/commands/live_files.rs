use crate::common::out;
use crate::graph::{Graph, Lang};

/// Prints every live file, one per line.
pub fn cmd_live_files<L: Lang>(graph: &Graph<L>) -> Result<u8, String> {
  graph.live()?.iter().for_each(|l| out(l));
  Ok(0)
}
