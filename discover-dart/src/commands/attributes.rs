//! `attributes`: the instance fields of a class.

use crate::dart::{self, DartRoot};
use crate::graph::Graph;
use discover_core::cli;
use discover_core::common::{file_arg, out, out_json};

/// One instance field of a class, as `attributes` reports it.
#[derive(Debug)]
pub struct Attribute {
  /// The declared type as written (`String`, `int?`, `List<String>`), or `dynamic` when the declaration has none.
  pub ty: String,
  pub name: String,
}

/// The instance fields declared in the body of class `class`, in source order, or None when the file declares no such class. Static fields, getters, setters, methods and constructors are not attributes. A declaration that names several fields (`int? a, b;`) gives one attribute per name.
///
/// TODO: add a flag that also lists inherited fields, from the class's `extends` superclass and `with` mixins, resolved through the import graph.
pub fn attributes(root: &DartRoot, class: &str) -> Option<Vec<Attribute>> {
  // Finds the class declaration with the requested name.
  let decl = root
    .root()
    .dfs()
    .find(|n| n.kind() == "class_declaration" && n.field("name").is_some_and(|name| name.text() == class))?;
  // Holds the fields found so far.
  let mut found = Vec::new();
  // A class without a body (`class A = B with C;`) declares no fields.
  let Some(body) = decl.field("body") else {
    return Some(found);
  };
  // Visits each member of the body; members are wrapped in class_member nodes.
  for member in body.children().filter(|c| c.kind() == "class_member") {
    // Field declarations are `declaration` nodes; methods, getters and setters are method_declaration.
    for declaration in member.children().filter(|c| c.kind() == "declaration") {
      // Skips static fields, which belong to the class rather than its instances.
      if declaration.children().any(|c| c.kind() == "static") {
        continue;
      }
      // Finds the list of declared names; constructors and other declarations have none.
      let Some(list) = declaration
        .children()
        .find(|c| matches!(c.kind().as_ref(), "initialized_identifier_list" | "static_final_declaration_list"))
      else {
        continue;
      };
      // Reads the declared type, which is a direct child of the declaration.
      let ty = declaration
        .children()
        .find(|c| c.kind() == "type")
        .map_or_else(|| "dynamic".to_string(), |t| t.text().to_string());
      // Reports each declarator in the list under the shared type.
      for declarator in list
        .children()
        .filter(|c| matches!(c.kind().as_ref(), "initialized_identifier" | "static_final_declaration"))
      {
        // The declarator's text starts with the declared name.
        let text = declarator.text();
        let name: String = text.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect();
        found.push(Attribute {
          ty: ty.clone(),
          name: if name.is_empty() { "?".to_string() } else { name },
        });
      }
    }
  }
  Some(found)
}

// ─────────────────────────────────────────────────────────
// attributes — the instance fields of a class: one name per
// line, or with --verbose a JSON array of { <name>: <type> }
// ─────────────────────────────────────────────────────────
pub fn cmd_attributes(graph: &Graph, args: &[String]) -> Result<u8, String> {
  // Takes the class name and the file that defines it.
  let [class, file] = args else {
    return Err("usage: discover --lang=dart attributes <class> <file>".into());
  };
  let (path, _) = file_arg(&graph.project.root, file)?;
  // Parses the one file; the cached facts don't keep class members.
  let src = std::fs::read_to_string(&path).map_err(|e| format!("{file}: {e}"))?;
  let root = dart::parse(&src);
  let attributes = attributes(&root, class).ok_or_else(|| format!("class {class} not found in {file}"))?;
  if *cli::VERBOSE {
    // One single-key object per field, name to type, in source order.
    let typed: Vec<std::collections::BTreeMap<&str, &str>> =
      attributes.iter().map(|a| std::collections::BTreeMap::from([(a.name.as_str(), a.ty.as_str())])).collect();
    out_json(&typed);
  } else {
    // Plain text, one name per line.
    attributes.iter().for_each(|a| out(&a.name));
  }
  Ok(0)
}
