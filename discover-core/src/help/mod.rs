//! Help pages, laid out like `gh`'s: `discover --help` prints the top-level page, whose COMMANDS section lists each subcommand with its summary, and `discover COMMAND --help` prints that subcommand's own page. Every page follows the man page format: section headings at the margin, text indented 7 columns, entry descriptions 14, each paragraph one line in the source; [`fill`] breaks paragraphs to the line width when printed.

pub mod attributes;
pub mod config;
pub mod dead_files;
pub mod definition;
pub mod endpoints;
pub mod importers;
pub mod inspect;
pub mod is_live;
pub mod page;
pub mod prune;
pub mod resolve;
pub mod trace;
pub mod usages;

/// One subcommand's page. A language overrides a field by building its page from core's with struct update syntax (`Page { description: "...", ..trace::trace() }`).
#[derive(Clone, Copy)]
pub struct Page {
  /// What the subcommand does, in a few words: the rest of the page's NAME line, and the subcommand's line in the top-level COMMANDS section.
  pub summary: &'static str,
  /// One usage per form, without the leading `discover`.
  pub synopsis: &'static [&'static str],
  /// The DESCRIPTION section, one paragraph per element.
  pub description: &'static [&'static str],
  /// The OPTIONS section: each option this subcommand takes, or global option it gives a behavior of its own, with its description.
  pub options: &'static [(&'static str, &'static str)],
}

/// A language's help: the subcommands it implements and, for each, its page. Every method defaults to core's page; a language overrides one only where its implementation has something specific to say.
pub trait Help {
  /// The subcommands this language implements, in the order its COMMANDS section lists them.
  const COMMANDS: &'static [&'static str];

  fn resolve() -> Page {
    resolve::resolve()
  }
  fn inspect() -> Page {
    inspect::inspect()
  }
  fn definition() -> Page {
    definition::definition()
  }
  fn usages() -> Page {
    usages::usages()
  }
  fn attributes() -> Page {
    attributes::attributes()
  }
  fn endpoints() -> Page {
    endpoints::endpoints()
  }
  fn importers() -> Page {
    importers::importers()
  }
  fn trace() -> Page {
    trace::trace()
  }
  fn prune() -> Page {
    prune::prune()
  }
  fn dead_files() -> Page {
    dead_files::dead_files()
  }
  fn is_live() -> Page {
    is_live::is_live()
  }
}

/// Core's generic pages, for when no language is known: every public subcommand, as core describes it.
pub struct Generic;

impl Help for Generic {
  const COMMANDS: &'static [&'static str] = &["resolve", "inspect", "definition", "usages", "attributes", "endpoints", "importers", "trace", "prune", "dead-files", "is-live"];
}

/// The page of `command` for language `H`: `config` for every language, any other subcommand only when `H` implements it.
pub fn page<H: Help>(command: &str) -> Option<Page> {
  // `config` runs before any language is chosen, so every language has it.
  if command == "config" {
    return Some(config::config());
  }
  // Leaves out a subcommand the language doesn't implement.
  if !H::COMMANDS.contains(&command) {
    return None;
  }
  Some(match command {
    "resolve" => H::resolve(),
    "inspect" => H::inspect(),
    "definition" => H::definition(),
    "usages" => H::usages(),
    "attributes" => H::attributes(),
    "endpoints" => H::endpoints(),
    "importers" => H::importers(),
    "trace" => H::trace(),
    "prune" => H::prune(),
    "dead-files" => H::dead_files(),
    "is-live" => H::is_live(),
    _ => return None,
  })
}

/// Renders one subcommand's page: NAME, SYNOPSIS, DESCRIPTION and, when it has any, OPTIONS.
pub fn render(command: &str, page: &Page) -> String {
  // NAME: the command and its summary.
  let mut text = format!("NAME\n       discover {command} - {}\n\nSYNOPSIS\n", page.summary);
  // SYNOPSIS: one line per form.
  for form in page.synopsis {
    text.push_str(&format!("       discover {form}\n"));
  }
  // DESCRIPTION: one paragraph per element, separated by blank lines.
  text.push_str("\nDESCRIPTION\n");
  text.push_str(&page.description.iter().map(|p| format!("       {p}")).collect::<Vec<_>>().join("\n\n"));
  // OPTIONS: an entry per option, its description indented below it.
  if !page.options.is_empty() {
    text.push_str("\n\nOPTIONS\n");
    text.push_str(&page.options.iter().map(|(flag, what)| format!("       {flag}\n              {what}")).collect::<Vec<_>>().join("\n\n"));
  }
  text
}

/// Renders the top-level page, its COMMANDS section listing `config` and then each of `commands` with the summary from its page.
pub fn render_top(commands: &[(&str, Page)]) -> String {
  // Lists `config` first, as it runs before any language is chosen.
  let config = ("config", config::config());
  let entries: Vec<String> = std::iter::once(&config)
    .chain(commands.iter())
    .map(|(name, page)| format!("       {name}\n              {}", page.summary))
    .collect();
  format!("{}\n\nCOMMANDS\n{}\n\n{}\n\n{}", page::HEAD, page::COMMANDS_INTRO, entries.join("\n\n"), page::TAIL)
}

/// Fills a help page to the line width the way man(1) renders a page: each line keeps its leading indent, and one longer than the width is broken between words into lines that repeat that indent. The width is $MANWIDTH, else $COLUMNS, else 80, as man uses.
pub fn fill(text: &str) -> String {
  // Reads the width from the environment, falling back to 80 columns.
  let width = ["MANWIDTH", "COLUMNS"]
    .iter()
    .find_map(|var| std::env::var(var).ok().and_then(|value| value.parse::<usize>().ok()))
    .filter(|&width| width > 0)
    .unwrap_or(80);
  // Holds the filled lines.
  let mut filled = Vec::new();
  for line in text.lines() {
    // Keeps a line that already fits exactly as written.
    if line.chars().count() <= width {
      filled.push(line.to_string());
      continue;
    }
    // Splits off the indent, which every continuation line repeats.
    let body = line.trim_start();
    let indent = &line[..line.len() - body.len()];
    // Holds the line being filled and whether it has a word on it yet.
    let mut current = indent.to_string();
    let mut has_word = false;
    for word in body.split_whitespace() {
      // Starts a new line when the word would run past the width; a word longer than the width gets a line to itself.
      if has_word && current.chars().count() + 1 + word.chars().count() > width {
        filled.push(std::mem::replace(&mut current, indent.to_string()));
        has_word = false;
      }
      // Separates the word from the one before it on the same line.
      if has_word {
        current.push(' ');
      }
      current.push_str(word);
      has_word = true;
    }
    filled.push(current);
  }
  filled.join("\n")
}
