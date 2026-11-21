use serde::Serialize;

pub struct InspectArgs<'a> {
  /// Whether each of the language's section flags was given, in order.
  pub sections: Vec<bool>,
  pub entities: bool,
  /// name | names | signatures | full
  pub depth: String,
  pub file: Option<&'a str>,
}

/// `inspect <file> [<section flags>] [--entities] [--depth names|signatures|full]`, in any order.
/// With no section and no --entities, everything is selected.
pub fn parse_inspect<'a>(args: &'a [String], section_flags: &[&str]) -> InspectArgs<'a> {
  let mut sections = vec![false; section_flags.len()];
  let mut entities = false;
  // Empty without --depth: entities print as JSON with kind, name and line.
  let mut depth = String::new();
  let mut file = None;
  let mut i = 0;
  while i < args.len() {
    let arg = args[i].as_str();
    if let Some(at) = section_flags.iter().position(|f| *f == arg) {
      sections[at] = true;
    } else {
      match arg {
        "--entities" => entities = true,
        "--depth" => {
          i += 1;
          depth = args.get(i).cloned().unwrap_or_default();
        }
        other => file = Some(other),
      }
    }
    i += 1;
  }
  if !(entities || sections.iter().any(|s| *s)) {
    sections.iter_mut().for_each(|s| *s = true);
    entities = true;
  }
  InspectArgs {
    sections,
    entities,
    depth,
    file,
  }
}

/// One entity in inspect's output.
#[derive(Serialize)]
pub struct EntityOut<'a> {
  pub kind: &'a str,
  pub name: &'a str,
  pub line: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub text: Option<&'a str>,
}

impl<'a> EntityOut<'a> {
  /// `text` is the header for signatures depth, the whole declaration for
  /// full depth, and omitted otherwise.
  pub fn new(kind: &'a str, name: &'a str, line: usize, header: &'a str, full: &'a str, depth: &str) -> Self {
    let text = match depth {
      "full" => Some(full),
      "signatures" => Some(header),
      _ => None,
    };
    EntityOut { kind, name, line, text }
  }
}
