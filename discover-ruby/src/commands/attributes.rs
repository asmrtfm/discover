//! `attributes <class> <file> [-v|--verbose]`: what instances of a class carry. For an ActiveRecord model, what `Model.attribute_names` gives (its table's columns in db/schema.rb and its `attribute` declarations) and its associations; --verbose adds its `attr_accessor`/`attr_reader`/`attr_writer` names and prints every entry with its type. For any other class, its `attr_*` names and `attribute` declarations always.

use crate::graph::Graph;
use crate::rb::{self, Directive, RbNode, RbRoot, call_args, is_constant_path, method_of, qualify, squeeze, string_value};
use discover_core::cli;
use discover_core::common::{file_arg, out};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Superclasses that make a class an ActiveRecord model.
const RECORD_BASES: &[&str] = &["ApplicationRecord", "ActiveRecord::Base"];

/// Words Rails' inflector leaves alone in both directions.
const UNCOUNTABLE: &[&str] = &["equipment", "information", "rice", "money", "species", "series", "fish", "sheep", "jeans", "police"];

/// Rails' irregular inflections, as (singular, plural).
const IRREGULAR: &[(&str, &str)] = &[("person", "people"), ("man", "men"), ("child", "children"), ("sex", "sexes"), ("move", "moves"), ("zombie", "zombies")];

pub fn run(graph: &Graph, args: &[String]) -> Result<u8, String> {
  // Takes the class name and the file that defines it.
  let [class, file] = args else {
    return Err("usage: discover --lang=ruby attributes <class> <file>".into());
  };
  let (path, _) = file_arg(&graph.project.root, file)?;
  let body = class_in(&path, class).ok_or_else(|| format!("class {class} not found in {file}"))?;
  let verbose = *cli::VERBOSE;
  let acronyms = fs::read_to_string(graph.project.root.join("config/initializers/inflections.rb")).map(|src| rb::acronyms(&rb::parse(&src))).unwrap_or_default();

  // Name and type of each attribute, in output order.
  let mut attributes: Vec<(String, String)> = Vec::new();
  let record = record_chain(graph, &path, class);
  // Columns and `attribute` declarations are typed with the Ruby class their values read as.
  if let Some(chain) = &record {
    attributes.extend(columns(graph, chain).into_iter().map(|(name, ty)| (name, ruby_type(&ty))));
  }
  attributes.extend(body.attributes.iter().map(|(name, ty)| (name.clone(), ruby_type(ty))));
  for association in &body.associations {
    attributes.push((association.name.clone(), association_type(association, &acronyms)));
  }
  // On a model the accessors are extras next to its columns, so only --verbose lists them.
  if record.is_none() || verbose {
    attributes.extend(body.accessors.iter().map(|a| (a.clone(), "untyped".to_string())));
  }
  // An attr_reader and attr_writer of one name, or an attribute over a column, is one attribute.
  let mut seen = HashSet::new();
  attributes.retain(|(name, _)| seen.insert(name.clone()));

  if verbose {
    // A JSON array of single-key objects, name to type, each object on one line (`{ "id": "Integer" },`).
    let quote = |s: &str| serde_json::to_string(s).unwrap_or_default();
    let rows: Vec<String> = attributes.iter().map(|(name, ty)| format!("  {{ {}: {} }}", quote(name), quote(ty))).collect();
    out(&if rows.is_empty() { "[]".to_string() } else { format!("[\n{}\n]", rows.join(",\n")) });
  } else {
    // Plain text, one name per line.
    attributes.iter().for_each(|(name, _)| out(name));
  }
  Ok(0)
}

/// The body of the class named `class` in a file, parsed afresh: the cached facts don't keep class bodies.
fn class_in(path: &Path, class: &str) -> Option<ClassBody> {
  let src = fs::read_to_string(path).ok()?;
  class_body(&rb::parse_file(path, &src), class)
}

/// When the class is an ActiveRecord model, its superclass chain up to ApplicationRecord or ActiveRecord::Base, as (class name, body), the class itself first; None when the chain ends anywhere else or can't be followed. Parses its own copy of each body, so the caller keeps theirs.
fn record_chain(graph: &Graph, path: &Path, class: &str) -> Option<Vec<(String, ClassBody)>> {
  let mut chain: Vec<(String, ClassBody)> = Vec::new();
  let (mut path, mut class, mut body): (PathBuf, String, ClassBody) = (path.to_path_buf(), class.to_string(), class_in(path, class)?);
  // Bounded, so a superclass cycle can't loop forever.
  for _ in 0..32 {
    let superclass = body.superclass.clone()?;
    chain.push((class, body));
    if RECORD_BASES.contains(&superclass.trim_start_matches("::")) {
      return Some(chain);
    }
    // The file the superclass link points at, then the superclass's body there.
    let target = graph.links(&path).iter().find(|l| l.directive == Directive::Inherit && l.uri == superclass).map(|l| l.target.clone())?;
    body = class_in(&target, &superclass)?;
    (path, class) = (target, superclass);
  }
  None
}

/// The columns of a model's table in db/schema.rb: the table of its topmost concrete class (single-table inheritance shares it), named by its `self.table_name` or else by Rails' convention (the class name, underscored and pluralized). Warns when the schema has no such table.
fn columns(graph: &Graph, chain: &[(String, ClassBody)]) -> Vec<(String, String)> {
  let Some((class, body)) = chain.iter().rev().find(|(_, b)| !b.abstract_class) else { return Vec::new() };
  // A namespaced model's table is named after the class alone (`Admin::User` → `users`).
  let short = class.rsplit("::").next().unwrap_or(class);
  let table = body.table_name.clone().unwrap_or_else(|| pluralize(&underscore(short)));
  let schema = graph.project.root.join("db/schema.rb");
  let Ok(src) = fs::read_to_string(&schema) else {
    eprintln!("warning: db/schema.rb not found; listing no columns");
    return Vec::new();
  };
  schema_columns(&rb::parse(&src), &table).unwrap_or_else(|| {
    eprintln!("warning: table {table} not found in db/schema.rb");
    Vec::new()
  })
}

/// The Ruby class a schema or `attribute` type reads as (`bigint` → `Integer`, `datetime` → `DateTime`); an array column (`string[]`) is `Array<String>`, and a type with no known class keeps its name camelized.
fn ruby_type(ty: &str) -> String {
  if let Some(element) = ty.strip_suffix("[]") {
    return format!("Array<{}>", ruby_type(element));
  }
  match ty {
    "integer" | "bigint" | "smallint" | "big_integer" | "immutable_integer" => "Integer",
    "string" | "text" | "citext" | "uuid" | "binary" | "immutable_string" => "String",
    "datetime" | "timestamp" | "timestamptz" => "DateTime",
    "date" => "Date",
    "time" => "Time",
    "boolean" => "Boolean",
    "decimal" | "numeric" => "BigDecimal",
    "float" => "Float",
    "json" | "jsonb" | "hstore" => "Hash",
    "inet" | "cidr" => "IPAddr",
    "untyped" => "untyped",
    other => return rb::camelize(other, &BTreeMap::new()),
  }
  .to_string()
}

/// An association's type: its target class (`class_name:`, or the name camelized and, for a collection, singularized), as `Collection<X>` for has_many and has_and_belongs_to_many; `polymorphic` when the target is decided per record.
fn association_type(association: &Association, acronyms: &BTreeMap<String, String>) -> String {
  if association.polymorphic {
    return "polymorphic".to_string();
  }
  let collection = matches!(association.kind.as_str(), "has_many" | "has_and_belongs_to_many");
  let class = association.class_name.clone().unwrap_or_else(|| {
    let singular = if collection { singularize(&association.name) } else { association.name.clone() };
    rb::camelize(&singular, acronyms)
  });
  if collection { format!("Collection<{class}>") } else { class }
}

/// Rails' `underscore` for a class name without namespace (`VenueManager` → `venue_manager`, `APIClient` → `api_client`).
fn underscore(name: &str) -> String {
  let chars: Vec<char> = name.chars().collect();
  let mut out = String::new();
  for (i, c) in chars.iter().enumerate() {
    if c.is_uppercase() && i > 0 {
      let prev = chars[i - 1];
      let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
      // A word starts after a lowercase letter or digit, or at the last capital of an acronym run (`APIClient` → `api_client`).
      if prev.is_lowercase() || prev.is_ascii_digit() || (prev.is_uppercase() && next_lower) {
        out.push('_');
      }
    }
    out.extend(c.to_lowercase());
  }
  out
}

/// Rails' `pluralize` for the common cases: uncountables, irregulars, `-y` → `-ies`, sibilants → `-es`, else `-s`.
fn pluralize(word: &str) -> String {
  if UNCOUNTABLE.iter().any(|u| word.ends_with(u)) {
    return word.to_string();
  }
  if let Some((singular, plural)) = IRREGULAR.iter().find(|(s, _)| word.ends_with(s)) {
    return format!("{}{plural}", &word[..word.len() - singular.len()]);
  }
  if let Some(stem) = word.strip_suffix('y').filter(|s| !s.ends_with(['a', 'e', 'i', 'o', 'u'])) {
    return format!("{stem}ies");
  }
  if ["s", "x", "z", "ch", "sh"].iter().any(|e| word.ends_with(e)) {
    return format!("{word}es");
  }
  format!("{word}s")
}

/// Rails' `singularize` for the common cases, the reverse of [`pluralize`].
fn singularize(word: &str) -> String {
  if UNCOUNTABLE.iter().any(|u| word.ends_with(u)) {
    return word.to_string();
  }
  if let Some((singular, plural)) = IRREGULAR.iter().find(|(_, p)| word.ends_with(p)) {
    return format!("{}{singular}", &word[..word.len() - plural.len()]);
  }
  if let Some(stem) = word.strip_suffix("ies") {
    return format!("{stem}y");
  }
  if let Some(stem) = ["sses", "xes", "zes", "ches", "shes", "uses"].iter().find_map(|e| word.strip_suffix(e).map(|s| format!("{s}{}", &e[..e.len() - 2]))) {
    return stem;
  }
  if word.ends_with("ss") {
    return word.to_string();
  }
  word.strip_suffix('s').unwrap_or(word).to_string()
}

/// A `belongs_to`, `has_one`, `has_many` or `has_and_belongs_to_many` declaration.
#[derive(Debug)]
pub struct Association {
  /// The macro: `belongs_to`, `has_one`, `has_many` or `has_and_belongs_to_many`.
  pub kind: String,
  pub name: String,
  /// The `class_name:` option, when given.
  pub class_name: Option<String>,
  /// `polymorphic: true`: the target class is decided per record.
  pub polymorphic: bool,
}

/// What a class body declares that its instances carry, each list in source order. Only statements directly in the body count, so nothing inside methods, blocks or `class << self`.
#[derive(Debug, Default)]
pub struct ClassBody {
  /// The superclass as written, when it is a constant path (`ApplicationRecord`, `ActiveRecord::Base`).
  pub superclass: Option<String>,
  /// `self.table_name = "x"`.
  pub table_name: Option<String>,
  /// `self.abstract_class = true` or `primary_abstract_class`: an ActiveRecord class with no table of its own.
  pub abstract_class: bool,
  /// `attribute :name, :type`, as (name, type); `untyped` when the type isn't a symbol.
  pub attributes: Vec<(String, String)>,
  /// The names `attr_accessor`, `attr_reader` and `attr_writer` declare.
  pub accessors: Vec<String>,
  pub associations: Vec<Association>,
}

/// The name a symbol or string argument holds (`:drinks`, `"drinks"`).
fn name_arg(arg: &RbNode<'_>) -> Option<String> {
  match arg.kind().as_ref() {
    "simple_symbol" => Some(arg.text().trim_start_matches(':').to_string()),
    _ => string_value(arg),
  }
}

/// The value of a call's keyword option `key` (`class_name: "X"`, `:class_name => "X"`, or either inside braces).
fn option<'r>(call: &RbNode<'r>, key: &str) -> Option<RbNode<'r>> {
  call_args(call)
    .into_iter()
    .flat_map(|arg| if arg.kind() == "hash" { arg.children().collect() } else { vec![arg] })
    .filter(|arg| arg.kind() == "pair")
    .find(|pair| pair.field("key").is_some_and(|k| k.text().trim_start_matches(':').trim_end_matches(':') == key))
    .and_then(|pair| pair.field("value"))
}

/// A class or module's name with its lexical namespace (`class B` inside `module A` → `A::B`).
fn qualified_name(node: &RbNode<'_>) -> Option<String> {
  let own = squeeze(&node.field("name")?.text());
  // The enclosing definitions' names, innermost first.
  let outer: Vec<String> = node.ancestors().filter(|a| matches!(a.kind().as_ref(), "class" | "module")).filter_map(|a| a.field("name").map(|n| squeeze(&n.text()))).collect();
  let namespace = outer.iter().rev().fold(None, |acc: Option<String>, written| Some(qualify(acc.as_ref(), written)));
  Some(qualify(namespace.as_ref(), &own))
}

/// The body of the first class named `class` (by its qualified name or its own name) in a file; None when the file defines no such class.
pub fn class_body(root: &RbRoot, class: &str) -> Option<ClassBody> {
  let wanted = class.strip_prefix("::").unwrap_or(class);
  let node = root.root().dfs().find(|n| {
    n.kind() == "class" && qualified_name(n).is_some_and(|q| q == wanted || q.rsplit("::").next() == Some(wanted))
  })?;
  let mut body = ClassBody {
    superclass: node.field("superclass").and_then(|s| s.children().find(is_constant_path)).map(|c| squeeze(&c.text())),
    ..ClassBody::default()
  };
  let Some(statements) = node.field("body") else { return Some(body) };
  for statement in statements.children() {
    match statement.kind().as_ref() {
      "call" => {
        let Some(method) = method_of(&statement, None) else { continue };
        let args = call_args(&statement);
        match method.as_str() {
          "attr_accessor" | "attr_reader" | "attr_writer" => body.accessors.extend(args.iter().filter_map(name_arg)),
          "attribute" => {
            if let Some(name) = args.first().and_then(name_arg) {
              // `attribute :x, :integer`; a type object (`ActiveModel::Type::Boolean.new`) is left untyped.
              let ty = args.get(1).filter(|t| t.kind() == "simple_symbol").and_then(name_arg).unwrap_or_else(|| "untyped".to_string());
              body.attributes.push((name, ty));
            }
          }
          "belongs_to" | "has_one" | "has_many" | "has_and_belongs_to_many" => {
            if let Some(name) = args.first().and_then(name_arg) {
              body.associations.push(Association {
                kind: method.clone(),
                name,
                class_name: option(&statement, "class_name").and_then(|v| name_arg(&v)),
                polymorphic: option(&statement, "polymorphic").is_some_and(|v| v.kind() == "true"),
              });
            }
          }
          "primary_abstract_class" => body.abstract_class = true,
          _ => {}
        }
      }
      // `self.table_name = "x"`, `self.abstract_class = true`.
      "assignment" => {
        let (Some(left), Some(right)) = (statement.field("left"), statement.field("right")) else { continue };
        match method_of(&left, Some("self")).as_deref() {
          Some("table_name") => body.table_name = name_arg(&right),
          Some("abstract_class") => body.abstract_class = right.kind() == "true",
          _ => {}
        }
      }
      _ => {}
    }
  }
  Some(body)
}

/// The columns `create_table "table"` declares in a db/schema.rb, as (name, type) in schema order, the primary key first; None when the schema has no such table. An `array: true` column's type ends in `[]`.
pub fn schema_columns(root: &RbRoot, table: &str) -> Option<Vec<(String, String)>> {
  let create = root.root().dfs().find(|n| n.kind() == "call" && method_of(n, None).as_deref() == Some("create_table") && call_args(n).first().and_then(string_value).as_deref() == Some(table))?;
  let mut columns: Vec<(String, String)> = Vec::new();
  // The primary key: `id: false` has none, `id: :uuid` sets its type, `primary_key: "x"` its name.
  let id = option(&create, "id");
  if id.as_ref().is_none_or(|v| v.kind() != "false") {
    let name = option(&create, "primary_key").and_then(|v| name_arg(&v)).unwrap_or_else(|| "id".to_string());
    columns.push((name, id.and_then(|v| name_arg(&v)).unwrap_or_else(|| "bigint".to_string())));
  }
  let Some(block) = create.field("block") else { return Some(columns) };
  // Each `t.<type> "name", ...` in the block; the schema nests nothing deeper.
  for call in block.dfs().filter(|n| n.kind() == "call" && n.field("receiver").is_some_and(|r| r.kind() == "identifier")) {
    let Some(method) = call.field("method").map(|m| m.text().to_string()) else { continue };
    let args = call_args(&call);
    let type_of = |ty: String| if option(&call, "array").is_some_and(|v| v.kind() == "true") { format!("{ty}[]") } else { ty };
    match method.as_str() {
      "timestamps" => columns.extend(["created_at", "updated_at"].map(|c| (c.to_string(), "datetime".to_string()))),
      "references" | "belongs_to" => {
        let Some(name) = args.first().and_then(name_arg) else { continue };
        let ty = option(&call, "type").and_then(|v| name_arg(&v)).unwrap_or_else(|| "bigint".to_string());
        columns.push((format!("{name}_id"), ty));
        if option(&call, "polymorphic").is_some_and(|v| v.kind() == "true") {
          columns.push((format!("{name}_type"), "string".to_string()));
        }
      }
      // Constraints and indexes are not columns.
      "index" | "check_constraint" | "foreign_key" | "exclusion_constraint" | "unique_constraint" => {}
      // `t.column "name", :type`.
      "column" => {
        if let (Some(name), Some(ty)) = (args.first().and_then(name_arg), args.get(1).and_then(name_arg)) {
          columns.push((name, type_of(ty)));
        }
      }
      _ => {
        if let Some(name) = args.first().and_then(string_value) {
          columns.push((name, type_of(method)));
        }
      }
    }
  }
  Some(columns)
}
