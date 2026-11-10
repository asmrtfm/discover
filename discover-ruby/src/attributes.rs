//! `attributes <class> <file> [-v|--verbose]`: what instances of a class carry. For an ActiveRecord model, what `Model.attribute_names` gives (its table's columns in db/schema.rb and its `attribute` declarations) and its associations; --verbose adds its `attr_accessor`/`attr_reader`/`attr_writer` names and prints every entry with its type. For any other class, its `attr_*` names and `attribute` declarations always.

use crate::graph::Graph;
use crate::rb::{self, ClassBody, Directive};
use discover_core::cli::{self, file_arg, out};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

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
  let verbose = cli::VERBOSE.load(Ordering::Relaxed);
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
  rb::class_body(&rb::parse_file(path, &src), class)
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
  rb::schema_columns(&rb::parse(&src), &table).unwrap_or_else(|| {
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
fn association_type(association: &rb::Association, acronyms: &BTreeMap<String, String>) -> String {
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
