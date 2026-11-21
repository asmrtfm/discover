//! Per-file facts read off a Ruby parse tree in one walk: the files it loads per directive, the constants it references (with the lexical nesting each is resolved in), its entities, and the names it uses.

use ast_grep_core::tree_sitter::{LanguageExt, StrDoc};
use ast_grep_core::{AstGrep, Node};
use ast_grep_language::SupportLang;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub type RbRoot = AstGrep<StrDoc<SupportLang>>;
pub(crate) type RbNode<'r> = Node<'r, StrDoc<SupportLang>>;

/// Whether a file is Ruby that discover parses: `.rb`, Rake files, Rack configs, gemspecs, Jbuilder views and ERB templates (whose embedded code is parsed), and the extensionless Rakefile and Gemfile.
pub fn is_ruby(path: &str) -> bool {
  let name = path.rsplit('/').next().unwrap_or(path);
  [".rb", ".rake", ".ru", ".gemspec", ".jbuilder", ".erb"].iter().any(|ext| name.ends_with(ext)) || matches!(name, "Rakefile" | "Gemfile")
}

/// Whether a file is an ERB template, whose Ruby lives between `<%` and `%>`.
pub fn is_erb(path: &Path) -> bool {
  path.extension().is_some_and(|e| e == "erb")
}

pub fn parse(src: &str) -> RbRoot {
  SupportLang::Ruby.ast_grep(src)
}

/// Parses a file's source: ERB templates through [`erb_code`], anything else as Ruby.
pub fn parse_file(path: &Path, src: &str) -> RbRoot {
  if is_erb(path) { parse(&erb_code(src)) } else { parse(src) }
}

/// The Ruby inside an ERB template, at the same lines and columns: template text and comment tags become spaces (newlines kept), and each closing `%>` becomes `;` so consecutive tags read as separate statements.
pub fn erb_code(src: &str) -> String {
  // Holds the code, built character for character so line numbers stay put.
  let mut out = String::with_capacity(src.len());
  // Walks the template as a string, so the tag markers can be matched as prefixes.
  let mut rest = src;
  // Tracks whether the walk is inside a tag, and whether that tag is a comment.
  let (mut in_tag, mut comment) = (false, false);
  while let Some(c) = rest.chars().next() {
    if !in_tag && rest.starts_with("<%%") {
      // `<%%` is a literal `<%` in the output, not a tag.
      out.push_str("   ");
      rest = &rest[3..];
    } else if !in_tag && rest.starts_with("<%") {
      // Opens a tag; its marker (`<%`, `<%=`, `<%-`, `<%#`) is blanked.
      in_tag = true;
      let marker = if rest[2..].starts_with(['=', '-', '#']) { 3 } else { 2 };
      comment = rest[2..].starts_with('#');
      out.push_str(&" ".repeat(marker));
      rest = &rest[marker..];
    } else if in_tag && (rest.starts_with("-%>") || rest.starts_with("%>")) {
      // Closes the tag with a statement separator, padded to the marker's width.
      let marker = if rest.starts_with('-') { 3 } else { 2 };
      out.push(';');
      out.push_str(&" ".repeat(marker - 1));
      rest = &rest[marker..];
      in_tag = false;
    } else {
      // Keeps code and every newline; blanks template text and comments.
      out.push(if c == '\n' || (in_tag && !comment) { c } else { ' ' });
      rest = &rest[c.len_utf8()..];
    }
  }
  out
}

/// How a file links to another:
///   require          — `require "x"`, searched on the load path
///   require_relative — `require_relative "x"`, relative to the requiring file
///   load             — `load "x.rb"`, the file name as given
///   autoload         — `autoload :X, "x"`, searched like require
///   constant         — a reference to a constant that Zeitwerk autoloads from a file
///   inherit          — `class A < B`: A's file inherits from the file defining B
///   include          — `include M`: the includer gains M's instance methods
///   extend           — `extend M`: the extender gains M's methods as singleton methods
///   prepend          — `prepend M`: like include, ahead of the includer's own methods
/// The last four are ancestors. Every directive but constant is transitive, like a Dart export: required code is loaded globally and inherited methods land on the inheritor, so what the target defines reaches whoever uses the linking file, and their users in turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Directive {
  Require,
  RequireRelative,
  Load,
  Autoload,
  Constant,
  Inherit,
  Include,
  Extend,
  Prepend,
}

impl Directive {
  pub fn as_str(self) -> &'static str {
    match self {
      Directive::Require => "require",
      Directive::RequireRelative => "require_relative",
      Directive::Load => "load",
      Directive::Autoload => "autoload",
      Directive::Constant => "constant",
      Directive::Inherit => "inherit",
      Directive::Include => "include",
      Directive::Extend => "extend",
      Directive::Prepend => "prepend",
    }
  }

  /// Whether the target's entities reach the linking file's users through this link: every directive but a plain constant reference.
  pub fn is_transitive(self) -> bool {
    self != Directive::Constant
  }

  /// The directive a name stands for, if any.
  pub fn named(name: &str) -> Option<Directive> {
    [
      Directive::Require,
      Directive::RequireRelative,
      Directive::Load,
      Directive::Autoload,
      Directive::Constant,
      Directive::Inherit,
      Directive::Include,
      Directive::Extend,
      Directive::Prepend,
    ]
    .into_iter()
    .find(|d| d.as_str() == name)
  }
}

/// A path argument of require, require_relative, load or autoload, and the 1-based line it sits on.
#[derive(Clone, Debug)]
pub struct Spec {
  pub directive: Directive,
  pub text: String,
  pub line: usize,
  /// Whether `text` is relative to the requiring file's directory: always for require_relative, and for the others when the argument is `File.expand_path("x", __dir__)` or `File.join(__dir__, "x")`.
  pub relative: bool,
}

/// A constant as referenced (`User`, `Admin::User`, `::User`), with the lexical nesting it is resolved in, innermost last (`["Admin", "Admin::UsersController"]`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConstRef {
  pub text: String,
  pub nesting: Vec<String>,
  pub line: usize,
}

/// A superclass or mixin: `class A < B` (inherit) or `include`/`extend`/`prepend B`, with the constant as written, the nesting it is resolved in and its 1-based line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ancestor {
  pub directive: Directive,
  pub text: String,
  pub nesting: Vec<String>,
  pub line: usize,
  /// What gains the ancestor: `class` or `module`, the innermost definition around the statement; empty at the top level.
  pub owner: &'static str,
}

#[derive(Debug)]
pub struct Entity {
  /// module | class | def | def self | constant
  pub kind: &'static str,
  /// The name as declared, without its namespace (`UsersController`, `valid?`).
  pub name: String,
  /// The name with its lexical namespace: `Admin::UsersController`, `Admin::UsersController#index` for an instance method, `.` for a singleton method, `::` for a constant.
  pub qualified: String,
  pub line: usize,
  pub header: String,
  pub full: String,
}

#[derive(Debug, Default)]
pub struct Facts {
  /// In source order.
  pub specs: Vec<Spec>,
  /// One per distinct constant and nesting, at its first line.
  pub constants: Vec<ConstRef>,
  /// Superclasses and mixins, in source order. Each is also among `constants`.
  pub ancestors: Vec<Ancestor>,
  /// Every module, class, method and constant the file declares, at any depth, sorted by line.
  pub entities: Vec<Entity>,
  /// Names the file uses, each with the 1-based lines where it appears: constants (each segment of `A::B`), method names, local names and symbols. Declared names are not uses, and neither is the constant named by a superclass or include/extend/prepend: that statement links to the code, it doesn't use it.
  pub idents: BTreeMap<String, BTreeSet<usize>>,
}

impl Facts {
  pub fn specs(&self, directive: Directive) -> impl Iterator<Item = &Spec> {
    self.specs.iter().filter(move |s| s.directive == directive)
  }

  /// The first entity named `symbol`, by its own name or its qualified name.
  pub fn entity(&self, symbol: &str) -> Option<&Entity> {
    self.entities.iter().find(|e| e.name == symbol || e.qualified == symbol)
  }

  pub fn entity_names(&self) -> Vec<String> {
    self.entities.iter().map(|e| e.name.clone()).collect()
  }
}

pub(crate) fn line_of(node: &RbNode<'_>) -> usize {
  node.start_pos().line() + 1
}

/// The text of a constant path without whitespace (`A :: B` → `A::B`).
pub(crate) fn squeeze(text: &str) -> String {
  text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Whether a node is a constant or a path of constants (`A`, `A::B`, `::A::B`), as opposed to a path through an expression (`obj.class::X`).
pub(crate) fn is_constant_path(node: &RbNode<'_>) -> bool {
  match node.kind().as_ref() {
    "constant" => true,
    "scope_resolution" => node.field("scope").is_none_or(|s| is_constant_path(&s)) && node.field("name").is_some_and(|n| n.kind() == "constant"),
    _ => false,
  }
}

/// The name a class, module or constant declared as `written` gets inside `outer` (`B` in `A` → `A::B`; `::B` → `B`).
pub(crate) fn qualify(outer: Option<&String>, written: &str) -> String {
  match (outer, written.strip_prefix("::")) {
    (_, Some(top)) => top.to_string(),
    (Some(outer), None) => format!("{outer}::{written}"),
    (None, None) => written.to_string(),
  }
}

/// The contents of a string literal without interpolation; None for anything else and for an empty string.
pub(crate) fn string_value(node: &RbNode<'_>) -> Option<String> {
  if node.kind() != "string" || node.children().any(|c| c.kind() == "interpolation") {
    return None;
  }
  node.children().find(|c| c.kind() == "string_content").map(|c| c.text().to_string())
}

/// The arguments of a call, without punctuation or comments.
pub(crate) fn call_args<'r>(call: &RbNode<'r>) -> Vec<RbNode<'r>> {
  call
    .field("arguments")
    .map(|args| args.children().filter(|c| c.is_named() && c.kind() != "comment").collect())
    .unwrap_or_default()
}

/// The method a call invokes, when it has no receiver or the receiver is `receiver`.
pub(crate) fn method_of(call: &RbNode<'_>, receiver: Option<&str>) -> Option<String> {
  let got = call.field("receiver").map(|r| r.text().to_string());
  (got.as_deref() == receiver).then(|| call.field("method").map(|m| m.text().to_string())).flatten()
}

/// A path argument as (path, relative to the file's directory): a plain string; `File.expand_path("x", __dir__)` and `File.join(__dir__, "x", ...)`; or `File.expand_path("../x", __FILE__)`, which names `x` beside the file. None for anything computed.
fn path_arg(arg: &RbNode<'_>) -> Option<(String, bool)> {
  if let Some(text) = string_value(arg) {
    return Some((text, false));
  }
  if arg.kind() != "call" {
    return None;
  }
  let args = call_args(arg);
  match (method_of(arg, Some("File")).as_deref(), &args[..]) {
    (Some("expand_path"), [path, anchor]) => {
      let path = string_value(path)?;
      match anchor.text().as_ref() {
        "__dir__" => Some((path, true)),
        // Relative to the file itself, so the first `..` steps from the file to its directory.
        "__FILE__" => match path.as_str() {
          ".." => Some((".".to_string(), true)),
          _ => path.strip_prefix("../").map(|rest| (rest.to_string(), true)),
        },
        _ => None,
      }
    }
    (Some("join"), [anchor, parts @ ..]) if anchor.text() == "__dir__" && !parts.is_empty() => {
      let parts: Option<Vec<String>> = parts.iter().map(string_value).collect();
      Some((parts?.join("/"), true))
    }
    _ => None,
  }
}

/// The spec a call loads, when it is require, require_relative, load or autoload (bare or on Kernel).
fn loaded_spec(call: &RbNode<'_>) -> Option<Spec> {
  let method = method_of(call, None).or_else(|| method_of(call, Some("Kernel")))?;
  let args = call_args(call);
  let (directive, arg) = match method.as_str() {
    "require" => (Directive::Require, args.first()?),
    "require_relative" => (Directive::RequireRelative, args.first()?),
    "load" => (Directive::Load, args.first()?),
    "autoload" => (Directive::Autoload, args.get(1)?),
    _ => return None,
  };
  let (text, relative) = path_arg(arg)?;
  Some(Spec {
    directive,
    text,
    line: line_of(call),
    relative: relative || directive == Directive::RequireRelative,
  })
}

/// Builds facts in one recursive walk, carrying the lexical nesting.
struct Walk {
  facts: Facts,
  /// Distinct constant references seen so far, by text and nesting.
  seen: BTreeSet<(String, Vec<String>)>,
  /// The kinds (`class`, `module`) of the definitions around the walk, innermost last.
  owners: Vec<&'static str>,
}

impl Walk {
  fn use_name(&mut self, name: &str, line: usize) {
    self.facts.idents.entry(name.to_string()).or_default().insert(line);
  }

  /// Records a reference to a constant path and, when `used`, a use of each of its segments.
  fn constant(&mut self, node: &RbNode<'_>, nesting: &[String], used: bool) {
    let text = squeeze(&node.text());
    let line = line_of(node);
    if used {
      for segment in text.split("::").filter(|s| !s.is_empty()) {
        self.use_name(segment, line);
      }
    }
    if self.seen.insert((text.clone(), nesting.to_vec())) {
      self.facts.constants.push(ConstRef { text, nesting: nesting.to_vec(), line });
    }
  }

  /// Records a superclass or mixin, and its constant as a reference that is not a use.
  fn ancestor(&mut self, directive: Directive, constant: &RbNode<'_>, nesting: &[String], owner: &'static str) {
    self.facts.ancestors.push(Ancestor { directive, text: squeeze(&constant.text()), nesting: nesting.to_vec(), line: line_of(constant), owner });
    self.constant(constant, nesting, false);
  }

  fn entity(&mut self, node: &RbNode<'_>, kind: &'static str, name: String, qualified: String) {
    let full = node.text().to_string();
    let header = full.split('\n').next().unwrap_or("").to_string();
    self.facts.entities.push(Entity { kind, name, qualified, line: line_of(node), header, full });
  }

  /// Visits every child of `node` except those in `skip` (field names).
  fn children(&mut self, node: &RbNode<'_>, nesting: &mut Vec<String>, singleton: bool, skip: &[&str]) {
    let skipped: Vec<_> = skip.iter().filter_map(|f| node.field(f)).map(|n| n.range()).collect();
    for child in node.children() {
      if !skipped.contains(&child.range()) {
        self.visit(&child, nesting, singleton);
      }
    }
  }

  /// `singleton` is true inside `class << self`, where `def` declares singleton methods.
  fn visit(&mut self, node: &RbNode<'_>, nesting: &mut Vec<String>, singleton: bool) {
    match node.kind().as_ref() {
      "class" | "module" => {
        let Some(name) = node.field("name") else { return self.children(node, nesting, false, &[]) };
        let qualified = qualify(nesting.last(), &squeeze(&name.text()));
        let short = qualified.rsplit("::").next().unwrap_or(&qualified).to_string();
        let kind = if node.kind() == "class" { "class" } else { "module" };
        self.entity(node, kind, short, qualified.clone());
        // The superclass is resolved outside the class being opened.
        if let Some(superclass) = node.field("superclass") {
          // `class A < B` is an ancestor; `class A < Struct.new(:x)` is plain code.
          match superclass.children().find(is_constant_path) {
            Some(parent) => self.ancestor(Directive::Inherit, &parent, nesting, "class"),
            None => self.visit(&superclass, nesting, false),
          }
        }
        nesting.push(qualified);
        self.owners.push(kind);
        if let Some(body) = node.field("body") {
          self.visit(&body, nesting, false);
        }
        self.owners.pop();
        nesting.pop();
      }
      "singleton_class" => {
        // `class << self` makes its methods singleton methods; `class << obj` is left as plain code.
        let of_self = node.field("value").is_some_and(|v| v.kind() == "self");
        self.children(node, nesting, of_self, &[]);
      }
      "method" | "singleton_method" => {
        let Some(name) = node.field("name") else { return self.children(node, nesting, false, &[]) };
        let name = name.text().to_string();
        let on_class = singleton || node.kind() == "singleton_method";
        let owner = nesting.last().cloned().unwrap_or_default();
        let (kind, qualified) = if on_class { ("def self", format!("{owner}.{name}")) } else { ("def", format!("{owner}#{name}")) };
        self.entity(node, kind, name, qualified);
        self.children(node, nesting, false, &["name", "object"]);
      }
      "assignment" if node.field("left").is_some_and(|l| l.kind() == "constant") => {
        let name = node.field("left").map(|l| l.text().to_string()).unwrap_or_default();
        let qualified = qualify(nesting.last(), &name);
        self.entity(node, "constant", name, qualified);
        if let Some(right) = node.field("right") {
          self.visit(&right, nesting, singleton);
        }
      }
      "scope_resolution" if is_constant_path(node) => self.constant(node, nesting, true),
      // A path through an expression: only the expression can hold references.
      "scope_resolution" => {
        if let Some(scope) = node.field("scope") {
          self.visit(&scope, nesting, singleton);
        }
      }
      "constant" => self.constant(node, nesting, true),
      "call" => {
        if let Some(spec) = loaded_spec(node) {
          self.facts.specs.push(spec);
        }
        // `include A, B`, `extend A`, `prepend A` on the class being defined (no receiver, or self).
        let mixin = match method_of(node, None).or_else(|| method_of(node, Some("self"))).as_deref() {
          Some("include") => Some(Directive::Include),
          Some("extend") => Some(Directive::Extend),
          Some("prepend") => Some(Directive::Prepend),
          _ => None,
        };
        let Some(directive) = mixin else { return self.children(node, nesting, singleton, &[]) };
        // The mixed-in constants are ancestors; any other argument is plain code.
        let owner = self.owners.last().copied().unwrap_or("");
        for arg in call_args(node) {
          if is_constant_path(&arg) {
            self.ancestor(directive, &arg, nesting, owner);
          } else {
            self.visit(&arg, nesting, singleton);
          }
        }
        if let Some(receiver) = node.field("receiver") {
          self.visit(&receiver, nesting, singleton);
        }
      }
      "identifier" => self.use_name(&node.text(), line_of(node)),
      // `before_action :authenticate`, `send(:reset)`: a symbol names a method.
      "simple_symbol" => self.use_name(node.text().trim_start_matches(':'), line_of(node)),
      _ => self.children(node, nesting, singleton, &[]),
    }
  }
}

pub fn extract(root: &RbRoot) -> Facts {
  let mut walk = Walk { facts: Facts::default(), seen: BTreeSet::new(), owners: Vec::new() };
  walk.visit(&root.root(), &mut Vec::new(), false);
  let mut facts = walk.facts;
  // Stable, so entities on one line keep document order.
  facts.entities.sort_by_key(|e| e.line);
  facts
}

/// Rails' `camelize` for one path segment: each underscore-separated word capitalized, or replaced by its acronym (`api_client` → `APIClient` with the acronym `API`).
pub fn camelize(segment: &str, acronyms: &BTreeMap<String, String>) -> String {
  segment
    .split('_')
    .map(|word| match acronyms.get(word) {
      Some(acronym) => acronym.clone(),
      None => {
        let mut chars = word.chars();
        chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
      }
    })
    .collect()
}

/// The constant Zeitwerk expects a file to define, from its path below an autoload root (`admin/users_controller.rb` → `Admin::UsersController`).
pub fn constant_for(below_root: &str, acronyms: &BTreeMap<String, String>) -> Option<String> {
  let stem = below_root.strip_suffix(".rb")?;
  Some(stem.split('/').map(|s| camelize(s, acronyms)).collect::<Vec<_>>().join("::"))
}

/// The acronyms an inflections initializer declares (`inflect.acronym "API"`), keyed by their lowercase form.
pub fn acronyms(root: &RbRoot) -> BTreeMap<String, String> {
  root
    .root()
    .dfs()
    .filter(|n| n.kind() == "call" && n.field("method").is_some_and(|m| m.text() == "acronym"))
    .filter_map(|n| call_args(&n).first().and_then(string_value))
    .map(|a| (a.to_lowercase(), a))
    .collect()
}

/// What config/application.rb tells Zeitwerk: whether it calls `config.autoload_lib`, the lib subdirectories that call ignores, and the directories `autoload_paths` / `eager_load_paths` gain through `Rails.root.join(...)`. Commented-out lines are comments in the tree, so they don't count.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RailsConfig {
  pub autoload_lib: bool,
  pub lib_ignored: Vec<String>,
  pub extra_roots: Vec<String>,
}

pub fn rails_config(root: &RbRoot) -> RailsConfig {
  let mut config = RailsConfig::default();
  for node in root.root().dfs() {
    let kind = node.kind();
    if kind == "call" && node.field("method").is_some_and(|m| m.text() == "autoload_lib") {
      config.autoload_lib = true;
      // Every string under the arguments is an ignored subdirectory (`ignore: %w[assets tasks]`).
      for arg in call_args(&node) {
        config.lib_ignored.extend(arg.dfs().filter(|n| n.kind() == "string_content").map(|n| n.text().to_string()));
      }
    }
    // `config.autoload_paths << Rails.root.join("extras")`, `config.eager_load_paths += [...]`, `.push(...)`.
    let text = node.text();
    let adds = matches!(kind.as_ref(), "binary" | "operator_assignment" | "call") && (text.contains("autoload_paths") || text.contains("eager_load_paths"));
    let is_statement = node.parent().is_some_and(|p| matches!(p.kind().as_ref(), "body_statement" | "program" | "block_body" | "do_block"));
    if adds && is_statement {
      for join in node.dfs().filter(|n| n.kind() == "call" && n.field("method").is_some_and(|m| m.text() == "join") && n.field("receiver").is_some_and(|r| squeeze(&r.text()) == "Rails.root")) {
        let parts: Option<Vec<String>> = call_args(&join).iter().map(string_value).collect();
        if let Some(parts) = parts.filter(|p| !p.is_empty()) {
          config.extra_roots.push(parts.join("/"));
        }
      }
    }
  }
  config
}
