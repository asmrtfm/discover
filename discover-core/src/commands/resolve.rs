use crate::graph::Lang;

/// `resolve <what> [--from <file>]` → (what, from); from is "" when absent.
/// `what` names the argument in the usage message (`<uri>`, `<import>`, …).
pub fn parse_resolve<'a, L: Lang>(args: &'a [String], what: &str) -> Result<(&'a str, &'a str), String> {
  let mut uri = "";
  let mut from = "";
  let mut it = args.iter();
  while let Some(arg) = it.next() {
    match arg.as_str() {
      "--from" => from = it.next().map_or("", String::as_str),
      other => uri = other,
    }
  }
  if uri.is_empty() {
    return Err(format!("usage: {} resolve {what} [--from <file>]", L::NAME));
  }
  Ok((uri, from))
}
