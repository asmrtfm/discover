use crate::graph::Graph;
use crate::ts::{self, TsNode, TsRoot, fragment};
use discover_core::cli::VERBOSE;
use discover_core::common::{file_arg, out, out_json};

// ─────────────────────────────────────────────────────────
// attributes — the instance fields of a class, or the schema
// fields of a Mongoose model: one name per line, or with
// --verbose a JSON array of { <name>: <type> }
// ─────────────────────────────────────────────────────────
pub fn cmd_attributes(graph: &Graph, args: &[String]) -> Result<u8, String> {
  // Takes the class (or model) name and the file that defines it.
  let [class, file] = args else {
    return Err("usage: discover --lang=typescript attributes <class> <file>".into());
  };
  let (path, _) = file_arg(&graph.project.root, file)?;
  // Parses the one file; the cached facts don't keep class members or schemas.
  let src = std::fs::read_to_string(&path).map_err(|e| format!("{file}: {e}"))?;
  let root = ts::parse(&src, ts::lang_for(&path));
  let attributes = attributes(&root, class).ok_or_else(|| format!("class or model {class} not found in {file}"))?;
  if *VERBOSE {
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

/// One instance field of a class, as `attributes` reports it.
#[derive(Debug)]
pub struct Attribute {
  /// The declared type as written (`string`, `string[]`, `Map<string, number>`), or `any` when the field has no annotation.
  pub ty: String,
  pub name: String,
}

/// The attributes of `class`, in source order, or None when the file declares neither a class nor a Mongoose model by that name. A class gives its instance fields; a Mongoose model gives its schema's fields.
///
/// TODO: add a flag that also lists inherited fields, from the class's `extends` superclass (or a schema's `add`/discriminator base), resolved through the import graph.
pub fn attributes(root: &TsRoot, class: &str) -> Option<Vec<Attribute>> {
  class_attributes(root, class).or_else(|| mongoose_attributes(root, class))
}

/// The instance fields of class `class`, in source order, or None when the file declares no such class. Fields come from the class body (`name: string;`, `#secret: string;`) and from constructor parameter properties (`constructor(private id: string)`). Static fields, accessors and methods are not attributes.
fn class_attributes(root: &TsRoot, class: &str) -> Option<Vec<Attribute>> {
  // Finds the class declaration with the requested name, abstract or not.
  let decl = root.root().dfs().find(|n| {
    matches!(n.kind().as_ref(), "class_declaration" | "abstract_class_declaration") && n.field("name").is_some_and(|name| name.text() == class)
  })?;
  // Holds the fields found so far.
  let mut found = Vec::new();
  // Every class declaration has a body.
  let Some(body) = decl.field("body") else {
    return Some(found);
  };
  // Visits the members of the body in source order.
  for member in body.children() {
    match member.kind().as_ref() {
      // A field declared in the body; static ones belong to the class rather than its instances.
      "public_field_definition" if !member.children().any(|c| c.kind() == "static") => {
        if let Some(name) = member.field("name") {
          found.push(Attribute {
            ty: annotation(member.field("type")),
            name: name.text().to_string(),
          });
        }
      }
      // The constructor: parameters with an accessibility modifier or readonly are parameter properties, which declare instance fields.
      "method_definition" if member.field("name").is_some_and(|n| n.text() == "constructor") => {
        // Visits the constructor's parameters.
        for param in member.field("parameters").into_iter().flat_map(|p| p.children().collect::<Vec<_>>()) {
          // Only required and optional parameters can be parameter properties.
          if !matches!(param.kind().as_ref(), "required_parameter" | "optional_parameter") {
            continue;
          }
          // A plain parameter declares no field.
          if !param.children().any(|c| matches!(c.kind().as_ref(), "accessibility_modifier" | "readonly")) {
            continue;
          }
          if let Some(name) = param.field("pattern") {
            found.push(Attribute {
              ty: annotation(param.field("type")),
              name: name.text().to_string(),
            });
          }
        }
      }
      _ => {}
    }
  }
  Some(found)
}

/// The type in a `type_annotation` node without its leading colon, or `any` when there is no annotation.
fn annotation(node: Option<TsNode<'_>>) -> String {
  node.map_or_else(
    || "any".to_string(),
    |n| n.text().trim_start_matches(':').trim().to_string(),
  )
}

/// The fields of Mongoose model `model`, in schema order, or None when the file defines no such model. The model is a `model(...)` call (`mongoose.model('Profile', profileSchema)`) whose name argument is `model` or that is assigned to a variable named `model`. Its schema is the call's second argument: a `Schema(...)` / `new Schema(...)` expression, a variable holding one, or a plain definition object. Each top-level key of the definition is a field; `{ timestamps: true }` in the schema options adds `createdAt` and `updatedAt`. Virtuals are not stored, so they are not fields.
fn mongoose_attributes(root: &TsRoot, model: &str) -> Option<Vec<Attribute>> {
  // Finds the model call: by its name argument, or by the variable it is assigned to.
  let call = root.root().dfs().find(|n| {
    is_call_to(n, "model")
      && (call_args(n).first().is_some_and(|a| string_value(a).as_deref() == Some(model))
        || n.parent().is_some_and(|p| p.kind() == "variable_declarator" && p.field("name").is_some_and(|name| name.text() == model)))
  })?;
  // The schema is the second argument.
  let schema = call_args(&call).into_iter().nth(1)?;
  // Resolves a variable to the expression it is initialized with.
  let schema = if schema.kind() == "identifier" { variable_value(root, &schema.text())? } else { schema };
  // Splits a Schema(...) call into its definition and options; a bare object is a definition with no options.
  let (definition, options) = if is_call_to(&schema, "Schema") {
    let mut args = call_args(&schema).into_iter();
    (args.next()?, args.next())
  } else {
    (schema, None)
  };
  // Only an object literal can be read field by field.
  if definition.kind() != "object" {
    return None;
  }
  // Reads one field per key of the definition.
  let mut found: Vec<Attribute> = definition
    .children()
    .filter(|c| c.kind() == "pair")
    .filter_map(|pair| {
      Some(Attribute {
        name: key_name(&pair.field("key")?)?,
        ty: schema_type(&pair.field("value")?),
      })
    })
    .collect();
  // `timestamps: true` makes Mongoose add and maintain these two fields.
  let timestamps = options.is_some_and(|o| {
    o.kind() == "object"
      && o.children().any(|p| p.kind() == "pair" && p.field("key").and_then(|k| key_name(&k)).as_deref() == Some("timestamps") && p.field("value").is_some_and(|v| v.kind() == "true"))
  });
  if timestamps {
    for name in ["createdAt", "updatedAt"] {
      found.push(Attribute { ty: "Date".to_string(), name: name.to_string() });
    }
  }
  Some(found)
}

/// Whether `node` is a call or `new` expression whose callee is `name` or ends in `.name` (`model(...)`, `mongoose.model(...)`, `new mongoose.Schema(...)`).
fn is_call_to(node: &TsNode<'_>, name: &str) -> bool {
  // Reads the callee: `function` on a call, `constructor` on a new expression.
  let callee = match node.kind().as_ref() {
    "call_expression" => node.field("function"),
    "new_expression" => node.field("constructor"),
    _ => None,
  };
  callee.is_some_and(|c| {
    // A member callee is matched on its last property.
    let last = if c.kind() == "member_expression" { c.field("property").map(|p| p.text().to_string()) } else { Some(c.text().to_string()) };
    last.as_deref() == Some(name)
  })
}

/// The argument expressions of a call or `new` expression, without punctuation or comments.
fn call_args<'r>(node: &TsNode<'r>) -> Vec<TsNode<'r>> {
  node
    .field("arguments")
    .map(|args| args.children().filter(|c| c.is_named() && c.kind() != "comment").collect())
    .unwrap_or_default()
}

/// The contents of a string literal, or None for anything else.
fn string_value(node: &TsNode<'_>) -> Option<String> {
  (node.kind() == "string").then(|| fragment(node).map(|f| f.text().to_string()).unwrap_or_default())
}

/// The initializer of the variable named `name` (`const profileSchema = ...`), searching the whole file.
fn variable_value<'r>(root: &'r TsRoot, name: &str) -> Option<TsNode<'r>> {
  root
    .root()
    .dfs()
    .find(|n| n.kind() == "variable_declarator" && n.field("name").is_some_and(|id| id.text() == name))
    .and_then(|decl| decl.field("value"))
}

/// The name an object key spells: an identifier, a quoted string or a number. A computed key (`[expr]`) has no fixed name.
fn key_name(key: &TsNode<'_>) -> Option<String> {
  match key.kind().as_ref() {
    "string" => string_value(key),
    "property_identifier" | "number" => Some(key.text().to_string()),
    _ => None,
  }
}

/// The type a Mongoose schema value declares:
///   `String`, `mongoose.Schema.Types.ObjectId` → the last name (`String`, `ObjectId`)
///   `{ type: X, ... }`                         → the type of X
///   `[X]`                                      → `[<type of X>]`; `[]` → `Array`
///   any other object                           → `Object` (a nested path)
fn schema_type(value: &TsNode<'_>) -> String {
  match value.kind().as_ref() {
    "identifier" => value.text().to_string(),
    "member_expression" => value.field("property").map_or_else(|| value.text().to_string(), |p| p.text().to_string()),
    "object" => value
      .children()
      .find(|p| p.kind() == "pair" && p.field("key").and_then(|k| key_name(&k)).as_deref() == Some("type"))
      .and_then(|p| p.field("value"))
      .map_or_else(|| "Object".to_string(), |t| schema_type(&t)),
    "array" => value
      .children()
      .find(|c| c.is_named() && c.kind() != "comment")
      .map_or_else(|| "Array".to_string(), |element| format!("[{}]", schema_type(&element))),
    // Anything else (a call, a string) is shown as written.
    _ => value.text().to_string(),
  }
}
