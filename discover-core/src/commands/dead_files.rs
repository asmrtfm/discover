use crate::common::out;
use crate::graph::{Graph, Lang};

/// Prints every dead file, one per line, found by the backward walk.
pub fn cmd_dead_files<L: Lang>(graph: &Graph<L>) -> Result<u8, String> {
  graph.dead()?.iter().for_each(|l| out(l));
  Ok(0)
}
