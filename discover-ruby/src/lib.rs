//! discover-ruby — deterministic Ruby codebase navigation, in-process on ast-grep's Ruby support. Files link through require, require_relative, load and autoload, each resolved by its own rule; in projects that use Zeitwerk (Rails apps, gems calling `Zeitwerk::Loader.for_gem`), a reference to an autoloaded constant links to the file that defines it. The subcommands that only need the graph come from discover-core; the Ruby-specific ones are here.
//!
//! Subcommands:
//!   resolve <specifier|constant> [--from <file>] [--require|--require-relative|--load|--autoload]
//!   inspect <file> [--requires] [--ancestors] [--constants] [--entities] [--depth names|signatures|full]
//!   definition <symbol> [<file>] [-v|--verbose] [-p|--pretty]
//!   live-files
//!   dead-files [--rebuild]
//!   is-live <file>
//!   importers [--strict] <files ...>
//!   trace [--strict] [--include-upstream] [-v|--verbose] <files ...>
//!   prune [--strict] <files ...>
//!   usages <pattern> [--live-only]
//!   usages --assets <asset-path>
//!   attributes <class> <file> [-v|--verbose]

mod attributes;
mod graph;
pub mod project;
pub mod rb;
mod trace;
mod usages;

use discover_core::cli::{self, DefinitionArgs, DefinitionOut, EntityOut, UsagesArgs, file_arg, out, out_definition, out_json, out_pretty_definition, parse_definition, parse_inspect, parse_usages};
use graph::{Graph, RbLang};
use project::Project;
use rb::Directive;
use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::process::ExitCode;

/// Ruby's own commands, continuing the help page's COMMANDS section in its manual-page layout.
const USAGE: &str = "       resolve SPECIFIER [--from FILE] [--require|--require-relative|--load|--autoload]
              Print the repo-relative path a require-style SPECIFIER resolves to, by the rule of the given directive: require and autoload search lib/; require_relative resolves against FILE's directory; load takes the file name as given, from the project root or lib/. Without a directive flag, require_relative when --from is given, else require. A SPECIFIER that is a constant (Admin::User) resolves to the file Zeitwerk autoloads it from, looked up in FILE's namespaces when given.

       inspect FILE [--requires] [--ancestors] [--constants] [--entities] [--depth names|signatures|full]
              Print FILE's require, require_relative, load and autoload calls, its superclasses and include/extend/prepend mixins, the constants it references, and its modules, classes, methods and constants (at any depth) as JSON; with none of these flags, all of them, constants only in projects that autoload. --depth signatures adds each entity's header line and --depth full its whole declaration; --entities --depth names prints only the entity names, one per line.

       definition SYMBOL [FILE] [-p|--pretty]
              Print where SYMBOL, as used in FILE, is defined: among FILE's own entities, then the file Zeitwerk autoloads it from, then the files FILE requires or inherits from (superclasses and mixins), directly or through them, nearest first. SYMBOL is a name (User, valid?) or a qualified name (Admin::User, User#valid?, User.find_by_slug). Without FILE, print every definition of SYMBOL in the project. --verbose adds each definition's code; --pretty prints a FILE:LINE KIND SYMBOL header followed by the code as written, instead of JSON.

       trace [--include-upstream] [-v|--verbose] FILE...
              Ruby's trace, which replaces the shared one above. For each importer, entities_used lists where FILE's entities are used through it: in the importer itself; when the importer is a module, in every class and module downstream of it (those that include, extend, prepend or inherit it, through any number of such links); and when the importer or anything downstream of it is a class, at every callsite of that class in the codebase, ERB templates included. An entry found in another file has that file and a via list, the inheritance chain from the importer to it; each entry is printed on one line. A row is stale only when none of these finds a use. The constant named in a superclass or include/extend/prepend statement is not a use. --include-upstream also counts the entities FILE pulls in through its own requires, superclasses and mixins. --verbose also lists every file reached through transitive links (requires, superclasses, mixins) as a row of its own, with a via list of the files between it and FILE, nearest FILE first.

       usages PATTERN [--live-only]
              Print every match of the ast-grep PATTERN in the sources as JSON; with --live-only, only matches in live files.

       usages --assets PATH
              Print the lines of live Ruby files and ERB templates whose string literals name the asset at PATH: its repo path, its URL for a file under public/, or its logical path for a file under app/assets/<dir>/ (what image_tag takes).

       attributes CLASS FILE
              List what instances of CLASS, declared in FILE, carry, one name per line; with --verbose, a JSON array of { NAME: TYPE } objects, one per line. For an ActiveRecord model (a class whose superclass chain reaches ApplicationRecord or ActiveRecord::Base), the columns of its table in db/schema.rb (the table of its topmost concrete class, named by self.table_name or else the class name underscored and pluralized), its attribute declarations and its belongs_to/has_one/has_many/has_and_belongs_to_many associations; --verbose adds its attr_accessor, attr_reader and attr_writer names. For any other class, its attribute declarations, associations and attr_* names. Columns and attribute declarations are typed with the Ruby class their values read as (Integer, String, DateTime, Array<String> for an array column), associations with their target class (Collection<X> for has_many and has_and_belongs_to_many), and names without a known type are untyped.

       Links are require, require_relative, load and autoload calls whose path is a string literal, File.expand_path(\"x\", __dir__) or File.join(__dir__, \"x\"), and superclasses and include, extend and prepend mixins (directives inherit, include, extend, prepend), resolved through Zeitwerk or else to the nearest file in the require graph that declares them. Every link but a plain constant reference is transitive, like a Dart export: required code is loaded globally and inherited methods land on the inheritor, so a file passes on what it requires or inherits (a gem's lib/<name>.rb requiring its parts is a barrel). In trace such a row is tagged transitive. In a Rails app, or a gem that calls Zeitwerk::Loader.for_gem, a reference to an autoloaded constant also links to the file that defines it, and in Rails the entry points include everything under config/ and db/, controllers, channels, helpers, mailboxes and every ERB and Jbuilder template under app/, whose embedded Ruby is read for constants. Constants named only in strings or symbols (has_many :comments, const_get, constantize, class_name: \"X\") and computed require paths are not seen, so a file reached only that way is reported dead.";

/// How the discover binary identifies and runs Ruby.
pub const LANGUAGE: cli::Language = cli::Language {
  // Selects Ruby with --lang=ruby.
  name: "ruby",
  // Identifies Ruby source files.
  extensions: &["rb", "rake"],
  // Identifies a Ruby project.
  manifests: &["Gemfile", "gems.rb"],
  // Describes the default entry points in generated config files.
  entry_defaults: "Ruby executables in bin/ and exe/, lib/<name>.rb for each <name>.gemspec, Rakefile, config.ru and lib/tasks/**/*.rake; in Rails also config/**, db/**, controllers, channels, helpers, mailboxes and templates under app/; else the first of main.rb, app.rb, lib/main.rb, lib/app.rb",
  // Runs the function below.
  main,
  // Lists the Ruby commands for --help.
  usage: USAGE,
};

/// Runs the Ruby navigator.
pub fn main() -> ExitCode {
  // `trace` is Ruby's own (its --verbose and output layout); everything else is the shared command line.
  if let Some(args) = trace::trace_args() {
    return match run_trace(&args) {
      Ok(code) => ExitCode::from(code),
      Err(msg) => {
        eprintln!("error: {msg}");
        ExitCode::FAILURE
      }
    };
  }
  cli::main(USAGE, |_| Ok(graph::open(Project::discover()?)), |graph, command, args| match command {
    "resolve" => cmd_resolve(graph, args),
    "inspect" => cmd_inspect(graph, args),
    "definition" => cmd_definition(graph, args),
    "usages" => cmd_usages(graph, args),
    "attributes" => attributes::run(graph, args),
    // endpoints is Dart only for now: no output, exit 1.
    "endpoints" => Ok(1),
    _ => unreachable!("checked against COMMANDS"),
  })
}

/// Opens the graph, honouring --rebuild-cache and --no-rebuild like the shared command line, and runs Ruby's trace.
fn run_trace(args: &trace::TraceArgs) -> Result<u8, String> {
  let has = |flag: &str| std::env::args().any(|a| a == flag);
  let (rebuild, no_rebuild) = (has("--rebuild-cache"), has("--no-rebuild"));
  if rebuild && no_rebuild {
    return Err("--rebuild-cache and --no-rebuild cannot be combined".into());
  }
  if no_rebuild {
    discover_core::cache::skip_rebuild();
  }
  let graph = graph::open(Project::discover()?);
  if rebuild {
    graph.build_all()?;
  }
  trace::run(&graph, args)
}

/// Whether text names a constant path (`User`, `Admin::User`, `::User`) rather than a file.
fn is_constant(text: &str) -> bool {
  let path = text.strip_prefix("::").unwrap_or(text);
  !path.is_empty() && path.split("::").all(|s| s.starts_with(|c: char| c.is_ascii_uppercase()) && s.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

// ─────────────────────────────────────────────────────────
// resolve
// ─────────────────────────────────────────────────────────
fn cmd_resolve(graph: &Graph, args: &[String]) -> Result<u8, String> {
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

// ─────────────────────────────────────────────────────────
// inspect
// ─────────────────────────────────────────────────────────
#[derive(Serialize)]
struct RequireOut<'a> {
  directive: &'static str,
  uri: &'a str,
  line: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  resolved: Option<String>,
}

#[derive(Serialize)]
struct ConstantOut<'a> {
  constant: &'a str,
  line: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  resolved: Option<String>,
}

#[derive(Serialize)]
struct AncestorOut<'a> {
  directive: &'static str,
  constant: &'a str,
  line: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  resolved: Option<String>,
}

#[derive(Serialize)]
struct InspectOut<'a> {
  file: &'a str,
  #[serde(skip_serializing_if = "Option::is_none")]
  requires: Option<Vec<RequireOut<'a>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  ancestors: Option<Vec<AncestorOut<'a>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  constants: Option<Vec<ConstantOut<'a>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  entities: Option<Vec<EntityOut<'a>>>,
}

fn cmd_inspect(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let parsed = parse_inspect(args, &["--requires", "--ancestors", "--constants"]);
  let file = parsed.file.ok_or("usage: discover --lang=ruby inspect <file> [--requires] [--ancestors] [--constants] [--entities] [--depth names|signatures|full]")?;
  let [requires, ancestors, constants] = parsed.sections[..] else { unreachable!("one flag per section") };
  // Without Zeitwerk no constant resolves, so the list is only shown when asked for.
  let constants = constants && (graph.project.autoloads() || args.iter().any(|a| a == "--constants"));
  let (entities, depth) = (parsed.entities, parsed.depth.as_str());
  let (path, rel) = file_arg(&graph.project.root, file)?;
  let facts = graph.facts(&path);

  // --depth names: plain text, one name per line, no JSON wrapper.
  if entities && depth == "names" {
    facts.entities.iter().for_each(|e| out(&e.name));
    return Ok(0);
  }

  out_json(&InspectOut {
    file: &rel,
    requires: requires.then(|| {
      facts
        .specs
        .iter()
        .map(|s| RequireOut { directive: s.directive.as_str(), uri: &s.text, line: s.line, resolved: graph.project.resolve_spec(s, &rel) })
        .collect()
    }),
    ancestors: ancestors.then(|| {
      // Each ancestor resolved to the file its link points at.
      let links = graph.links(&path);
      facts
        .ancestors
        .iter()
        .map(|a| AncestorOut {
          directive: a.directive.as_str(),
          constant: &a.text,
          line: a.line,
          resolved: links.iter().find(|l| l.directive == a.directive && l.uri == a.text).map(|l| graph.project.rel(&l.target)),
        })
        .collect()
    }),
    constants: constants.then(|| {
      facts
        .constants
        .iter()
        .map(|c| ConstantOut { constant: &c.text, line: c.line, resolved: graph.project.resolve_constant(&c.text, &c.nesting).filter(|r| *r != rel) })
        .collect()
    }),
    entities: entities.then(|| facts.entities.iter().map(|e| EntityOut::new(e.kind, &e.name, e.line, &e.header, &e.full, depth)).collect()),
  });
  Ok(0)
}

// ─────────────────────────────────────────────────────────
// definition — with a file: its own entities, then the file
// the constant autoloads from, then the require graph below
// the file, nearest first. Without one: every definition of
// the symbol in the project
// ─────────────────────────────────────────────────────────
fn cmd_definition(graph: &Graph, args: &[String]) -> Result<u8, String> {
  let DefinitionArgs { symbol, file, verbose, pretty } = parse_definition::<RbLang>(args)?;
  let Some(file) = file else {
    return all_definitions(graph, symbol, verbose, pretty);
  };
  let (path, _) = file_arg(&graph.project.root, file)?;
  let facts = graph.facts(&path);

  // Prints the first entity matching the symbol in a file, if any.
  let found = |target: &PathBuf, via: Option<&[String]>, source: Option<&str>| -> bool {
    let facts = graph.facts(target);
    let Some(e) = facts.entity(symbol).or_else(|| symbol.rsplit("::").next().and_then(|last| facts.entity(last))) else {
      return false;
    };
    let target_rel = graph.project.rel(target);
    let def = DefinitionOut { symbol, kind: e.kind, file: &target_rel, line: e.line, via, source, code: verbose.then_some(e.full.as_str()) };
    out_definition(&def, pretty);
    true
  };

  if facts.entity(symbol).is_some() && found(&path, None, Some("local")) {
    return Ok(0);
  }

  // The constant (or a method's owner, `User#valid?`) as Zeitwerk would load it from this file.
  let owner = symbol.split(['#', '.']).next().unwrap_or(symbol);
  if is_constant(owner) {
    let nesting = facts.constants.iter().find(|c| c.text == owner).map(|c| c.nesting.clone()).unwrap_or_default();
    if let Some(target) = graph.project.resolve_constant(owner, &nesting).and_then(|r| graph.project.existing(&r)) {
      if found(&target, Some(&["(autoload)".to_string()]), None) {
        return Ok(0);
      }
    }
  }

  // Breadth-first down the require-style, superclass and mixin links, so the nearest definition wins; `via` is the chain of requiring or inheriting files.
  let mut visited: HashSet<PathBuf> = HashSet::from([path.clone()]);
  let mut queue: VecDeque<(PathBuf, Vec<String>)> = VecDeque::from([(path, Vec::new())]);
  while let Some((current, via)) = queue.pop_front() {
    for link in graph.links(&current).iter().filter(|l| l.directive != Directive::Constant).cloned() {
      if !visited.insert(link.target.clone()) {
        continue;
      }
      if found(&link.target, Some(&via), None) {
        return Ok(0);
      }
      let mut chain = via.clone();
      chain.push(graph.project.rel(&link.target));
      queue.push_back((link.target, chain));
    }
  }

  out_json(&serde_json::json!({ "symbol": symbol, "error": "not found in require graph" }));
  Ok(1)
}

/// Every declaration of `symbol`, by name or qualified name, in the project's graph files, in file order.
fn all_definitions(graph: &Graph, symbol: &str, verbose: bool, pretty: bool) -> Result<u8, String> {
  let files: Vec<(String, std::sync::Arc<rb::Facts>)> = graph.project.graph_files().iter().map(|f| (graph.project.rel(f), graph.facts(f))).collect();
  let found: Vec<DefinitionOut> = files
    .iter()
    .flat_map(|(rel, facts)| {
      facts.entities.iter().filter(|e| e.name == symbol || e.qualified == symbol).map(move |e| DefinitionOut {
        symbol,
        kind: e.kind,
        file: rel,
        line: e.line,
        via: None,
        source: None,
        code: verbose.then_some(e.full.as_str()),
      })
    })
    .collect();

  if found.is_empty() {
    out_json(&serde_json::json!({ "symbol": symbol, "error": "not found in project" }));
    return Ok(1);
  }
  if pretty {
    found.iter().for_each(out_pretty_definition);
  } else {
    out_json(&found);
  }
  eprintln!("{} definition(s) found", found.len());
  Ok(0)
}

// ─────────────────────────────────────────────────────────
// usages
// ─────────────────────────────────────────────────────────
fn cmd_usages(graph: &Graph, args: &[String]) -> Result<u8, String> {
  match parse_usages::<RbLang>(args)? {
    UsagesArgs::Assets(asset) => cli::print_usages(&usages::asset_usages(graph, asset)?),
    UsagesArgs::Pattern { pattern, live_only } => cli::print_usages(&usages::pattern_usages(graph, pattern, live_only)?),
  }
  Ok(0)
}
