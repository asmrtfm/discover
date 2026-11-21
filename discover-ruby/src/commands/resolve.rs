use crate::common::is_constant;
use crate::graph::Graph;
use crate::rb::Directive;
use discover_core::common::{file_arg, out};

/// Prints the repo-relative path a require-style specifier or a constant resolves to.
pub fn cmd_resolve(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let usage = "usage: discover --lang=ruby resolve <specifier|constant> [--from <file>] [--require|--require-relative|--load|--autoload]";
  let (mut spec, mut from, mut directive) = ("", "", None);
  let mut it = args.iter();
  while let Some(arg) = it.next() {
    match arg.as_str() {
      "--from" => from = it.next().map_or("", String::as_str),
      "--require" => directive = Some(Directive::Require),
      "--require-relative" => directive = Some(Directive::RequireRelative),
      "--load" => directive = Some(Directive::Load),
      "--autoload" => directive = Some(Directive::Autoload),
      other => spec = other,
    }
  }
  if spec.is_empty() {
    return Err(usage.into());
  }
  let resolved = if directive.is_none() && is_constant(spec) {
    // The namespaces the constant is referenced in, when FILE references it.
    let nesting = if from.is_empty() {
      Vec::new()
    } else {
      let (path, _) = file_arg(&graph.project.root, from)?;
      graph.facts(&path).constants.iter().find(|c| c.text == spec).map(|c| c.nesting.clone()).unwrap_or_default()
    };
    graph.project.resolve_constant(spec, &nesting)
  } else {
    let directive = directive.unwrap_or(if from.is_empty() { Directive::Require } else { Directive::RequireRelative });
    graph.project.resolve_path(directive, spec, directive == Directive::RequireRelative, from)
  };
  if let Some(resolved) = resolved {
    out(&resolved);
  }
  Ok(0)
}
