use crate::cli::VERBOSE;
use crate::common::{out, out_json};
use crate::graph::Lang;
use serde::Serialize;

#[derive(Serialize)]
pub struct DefinitionOut<'a> {
  pub symbol: &'a str,
  pub kind: &'a str,
  pub file: &'a str,
  pub line: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub via: Option<&'a [String]>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source: Option<&'a str>,
  /// The whole declaration, with `--verbose`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub code: Option<&'a str>,
}

pub struct DefinitionArgs<'a> {
  pub symbol: &'a str,
  /// The file the symbol is used in; without it every definition in the
  /// project is reported.
  pub file: Option<&'a str>,
  /// `-v` / `--verbose`: include each definition's code.
  pub verbose: bool,
  /// `-p` / `--pretty`: print each definition as a header line and its code
  /// as written instead of JSON. Implies `verbose`.
  pub pretty: bool,
}

/// `definition <symbol> [<file>] [-v|--verbose] [-p|--pretty]`.
pub fn parse_definition<'a, L: Lang>(args: &'a [String]) -> Result<DefinitionArgs<'a>, String> {
  let usage = || format!("usage: {} definition <symbol> [<file>] [-v|--verbose] [-p|--pretty]", L::NAME);
  let mut verbose = *VERBOSE;
  let mut pretty = false;
  let mut positional = Vec::new();
  for arg in args {
    match arg.as_str() {
      "-p" | "--pretty" => pretty = true,
      other => positional.push(other),
    }
  }
  // Pretty output is the code, so it needs the code.
  verbose |= pretty;
  match positional[..] {
    [symbol] => Ok(DefinitionArgs { symbol, file: None, verbose, pretty }),
    [symbol, file] => Ok(DefinitionArgs { symbol, file: Some(file), verbose, pretty }),
    _ => Err(usage()),
  }
}

/// `--pretty` output for one definition: a `file:line kind symbol` header
/// (with the re-export chain, when there is one), then its code as written.
pub fn out_pretty_definition(def: &DefinitionOut) {
  let mut header = format!("{}:{} {} {}", def.file, def.line, def.kind, def.symbol);
  if let Some(via) = def.via.filter(|v| !v.is_empty()) {
    header.push_str(&format!(" (via {})", via.join(" -> ")));
  }
  out(&header);
  out(def.code.unwrap_or_default());
  out("");
}

/// One definition, as JSON or with `--pretty`.
pub fn out_definition(def: &DefinitionOut, pretty: bool) {
  if pretty {
    out_pretty_definition(def);
  } else {
    out_json(def);
  }
}
