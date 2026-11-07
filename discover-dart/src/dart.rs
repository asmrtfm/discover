//! Per-file facts read straight off a Dart parse tree: the URIs of its
//! import/export/part directives and its top-level entities.
//!
//! The bash discover.sh found directives with the pattern `import '$URI';`,
//! which only matches single-quoted URIs with no trailing clauses. Here every
//! directive node is read directly, so `"double quoted"`, `as p`, `show`/`hide`,
//! `deferred as` and the alternatives of a conditional import are all seen.

use ast_grep_core::tree_sitter::{LanguageExt, StrDoc};
use ast_grep_core::{AstGrep, Node, Pattern};
use ast_grep_language::Dart;

pub type DartRoot = AstGrep<StrDoc<Dart>>;
type DartNode<'r> = Node<'r, StrDoc<Dart>>;

pub fn parse(src: &str) -> DartRoot {
  Dart.ast_grep(src)
}

/// The three linking directives that make up the import graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Directive {
  Import,
  Export,
  Part,
}

impl Directive {
  pub fn as_str(self) -> &'static str {
    match self {
      Directive::Import => "import",
      Directive::Export => "export",
      Directive::Part => "part",
    }
  }
}

/// A top-level declaration: a library member another file can reference.
#[derive(Debug)]
pub struct Entity {
  /// class | enum | mixin | extension | typedef | function | variable
  pub kind: &'static str,
  pub name: String,
  /// 1-based line of the node (for annotated declarations, the annotation's line).
  pub line: usize,
  /// The declaration's header line (the `--depth signatures` text).
  pub header: String,
  /// The whole declaration (the `--depth full` text).
  pub full: String,
}

/// Anything declared with a name, at any depth: every entity, plus the methods, accessors, constructors and local functions that are not entities.
#[derive(Debug)]
pub struct Definition {
  /// An entity's kind, or method | getter | setter | constructor
  pub kind: &'static str,
  pub name: String,
  /// 1-based line of the node.
  pub line: usize,
  /// The whole declaration.
  pub full: String,
}

#[derive(Debug, Default)]
pub struct Facts {
  pub imports: Vec<String>,
  pub exports: Vec<String>,
  /// The 1-based line of the `export` directive each of `exports` is in, index for index.
  pub export_lines: Vec<usize>,
  pub parts: Vec<String>,
  /// Sorted by line.
  pub entities: Vec<Entity>,
  /// Sorted by line.
  pub definitions: Vec<Definition>,
}

impl Facts {
  pub fn uris(&self, directive: Directive) -> &[String] {
    match directive {
      Directive::Import => &self.imports,
      Directive::Export => &self.exports,
      Directive::Part => &self.parts,
    }
  }

  /// The first definition (by line) with this name.
  pub fn definition(&self, name: &str) -> Option<&Definition> {
    self.definitions.iter().find(|d| d.name == name)
  }

  pub fn entity_names(&self) -> Vec<String> {
    self.entities.iter().map(|e| e.name.clone()).collect()
  }
}

/// Declaration node kinds and the keyword each is reported as, in the order
/// the bash version concatenated them (ties on a line keep this order).
const DECLARATION_KINDS: [(&str, &str); 5] = [
  ("class_declaration", "class"),
  ("enum_declaration", "enum"),
  ("mixin_declaration", "mixin"),
  ("extension_declaration", "extension"),
  ("type_alias", "typedef"),
];

pub fn extract(root: &DartRoot) -> Facts {
  let mut facts = Facts::default();
  let mut declarations: [Vec<Entity>; 5] = Default::default();
  let mut functions = Vec::new();
  let mut variables = Vec::new();
  let mut member_definitions = Vec::new();

  // One pre-order walk; directives and entities come out in document order.
  for node in root.root().dfs() {
    let kind = node.kind();
    match kind.as_ref() {
      "library_import" => facts.imports.extend(uris_in(&node)),
      "library_export" => {
        let uris = uris_in(&node);
        let line = node.start_pos().line() + 1;
        facts.export_lines.extend(std::iter::repeat_n(line, uris.len()));
        facts.exports.extend(uris);
      }
      "part_directive" => facts.parts.extend(uris_in(&node)),
      // Top-level functions only: methods are indented, so column 0 marks top level.
      "function_signature" if node.start_pos().column(&node) == 0 => {
        functions.push(function_entity(&node))
      }
      // Everything else with a signature has a definition without being an entity.
      "function_signature" => {
        let in_type = node.ancestors().any(|a| matches!(a.kind().as_ref(), "class_body" | "extension_body" | "enum_body"));
        member_definitions.push(member_definition(&node, if in_type { "method" } else { "function" }));
      }
      "getter_signature" => member_definitions.push(member_definition(&node, "getter")),
      "setter_signature" => member_definitions.push(member_definition(&node, "setter")),
      "constructor_signature" | "factory_constructor_signature" => {
        member_definitions.push(member_definition(&node, "constructor"))
      }
      "initialized_identifier" | "static_final_declaration" if is_top_level_variable(&node) => {
        variables.push(variable_entity(&node))
      }
      other => {
        if let Some(i) = DECLARATION_KINDS.iter().position(|(k, _)| *k == other) {
          declarations[i].push(declaration_entity(&node, DECLARATION_KINDS[i].1));
        }
      }
    }
  }

  let mut entities: Vec<Entity> = declarations
    .into_iter()
    .flatten()
    .chain(functions)
    .chain(variables)
    .collect();
  // Stable, so same-line entities keep the concatenation order above.
  entities.sort_by_key(|e| e.line);

  // Every entity has a definition, on top of the ones that aren't entities.
  let entity_definitions = entities.iter().map(|e| Definition {
    kind: e.kind,
    name: e.name.clone(),
    line: e.line,
    full: e.full.clone(),
  });
  let mut definitions: Vec<Definition> = entity_definitions.chain(member_definitions).collect();
  // Stable, so an entity stays ahead of a member on the same line.
  definitions.sort_by_key(|d| d.line);
  facts.definitions = definitions;
  facts.entities = entities;
  facts
}

/// URI strings under a directive. A conditional import has one `uri` for its
/// default and one per `if (...)` alternative; all of them are dependencies.
fn uris_in(directive: &DartNode<'_>) -> Vec<String> {
  directive
    .dfs()
    .filter(|n| n.kind() == "uri")
    .map(|n| unquote(&n.text()).to_string())
    .collect()
}

/// The contents of a Dart string literal: strips a raw `r` prefix and single,
/// double or triple quotes.
fn unquote(literal: &str) -> &str {
  let s = literal.trim();
  let s = s.strip_prefix('r').unwrap_or(s);
  for quote in ["'''", "\"\"\"", "'", "\""] {
    if let Some(inner) = s.strip_prefix(quote).and_then(|t| t.strip_suffix(quote)) {
      return inner;
    }
  }
  s
}

/// A variable declarator directly inside a top-level declaration:
///   const/final:    top_level_variable_declaration > static_final_declaration_list > static_final_declaration
///   var/typed/late: top_level_variable_declaration > initialized_identifier_list > initialized_identifier
/// Direct-parent chains only, so locals inside a top-level initializer are not picked up.
fn is_top_level_variable(node: &DartNode<'_>) -> bool {
  let list_kind = match node.kind().as_ref() {
    "initialized_identifier" => "initialized_identifier_list",
    _ => "static_final_declaration_list",
  };
  node
    .parent()
    .filter(|list| list.kind() == list_kind)
    .and_then(|list| list.parent())
    .is_some_and(|decl| decl.kind() == "top_level_variable_declaration")
}

fn first_line(text: &str) -> &str {
  text.split('\n').next().unwrap_or("")
}

/// Whether `line` contains `keyword` as a word that starts the declaration
/// header (at line start or after whitespace, followed by whitespace).
fn has_keyword(line: &str, keyword: &str) -> bool {
  line.match_indices(keyword).any(|(i, _)| {
    let before_ok = line[..i].chars().next_back().is_none_or(char::is_whitespace);
    let after_ok = line[i + keyword.len()..].chars().next().is_some_and(char::is_whitespace);
    before_ok && after_ok
  })
}

fn declaration_entity(node: &DartNode<'_>, keyword: &'static str) -> Entity {
  let full = node.text().to_string();
  // A typedef's name is a bare type_identifier (or, old-style, an identifier)
  // rather than a `name` field.
  let name = node
    .field("name")
    .or_else(|| node.children().find(|c| c.kind() == "type_identifier"))
    .or_else(|| {
      (keyword == "typedef")
        .then(|| node.children().find(|c| c.kind() == "identifier"))
        .flatten()
    })
    .map(|n| n.text().to_string())
    .unwrap_or_else(|| "?".to_string());
  // Leading annotations (`@JsonSerializable()`, `@riverpod`) belong to the
  // node, so the header is the first line with the keyword, not line one.
  let header = full
    .split('\n')
    .find(|l| has_keyword(l, keyword))
    .unwrap_or_else(|| first_line(&full))
    .to_string();
  Entity {
    kind: keyword,
    name,
    line: node.start_pos().line() + 1,
    header,
    full,
  }
}

fn function_entity(node: &DartNode<'_>) -> Entity {
  let full = node.text().to_string();
  Entity {
    kind: "function",
    name: node
      .field("name")
      .map(|n| n.text().to_string())
      .unwrap_or_else(|| "?".to_string()),
    line: node.start_pos().line() + 1,
    header: first_line(&full).to_string(),
    full,
  }
}

/// The definition behind a signature that isn't an entity. Its code is the whole method, body included; a signature with no enclosing method has only itself.
fn member_definition(node: &DartNode<'_>, kind: &'static str) -> Definition {
  Definition {
    kind,
    name: node
      .field("name")
      .map(|n| n.text().to_string())
      .unwrap_or_else(|| "?".to_string()),
    line: node.start_pos().line() + 1,
    full: node
      .ancestors()
      .find(|a| a.kind() == "method_declaration")
      .map_or_else(|| node.text().to_string(), |method| method.text().to_string()),
  }
}

fn variable_entity(node: &DartNode<'_>) -> Entity {
  let full = node.text().to_string();
  // The declarator's text starts with the declared name.
  let name: String = full
    .chars()
    .take_while(|c| c.is_alphanumeric() || *c == '_')
    .collect();
  Entity {
    kind: "variable",
    name: if name.is_empty() { "?".to_string() } else { name },
    line: node.start_pos().line() + 1,
    header: first_line(&full).to_string(),
    full,
  }
}

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

/// A stretch of a file that the code inside it belongs to, so a call written there can be traced to the callers of what encloses it: a top-level entity, or one member of a class, enum, mixin or extension.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Scope {
  /// The enclosing top-level entity's name.
  pub entity: String,
  /// The member's name (method, getter, setter or field), or None for the entity itself and for constructors, which are called by the entity's name.
  pub member: Option<String>,
  /// Whether the member is annotated `@override`, so it is called through a supertype (or by the framework, like `build`) rather than by its name.
  pub overrides: bool,
  /// The 1-based first line, leading annotations included.
  pub start: usize,
  /// The 1-based last line.
  pub end: usize,
}

/// Every top-level entity's scope and every member's scope, in document order. A member's scope lies inside its entity's, so the innermost scope around a line is the one with the fewest lines.
pub fn scopes(root: &DartRoot) -> Vec<Scope> {
  // Gives a node's 1-based first and last lines.
  let lines = |node: &DartNode<'_>| (node.start_pos().line() + 1, node.end_pos().line() + 1);
  let mut found = Vec::new();
  for node in root.root().children() {
    let kind = node.kind();
    // Classes, enums, mixins, extensions and typedefs, with the members in their bodies.
    if let Some((_, keyword)) = DECLARATION_KINDS.iter().find(|(k, _)| *k == kind.as_ref()) {
      let entity = declaration_entity(&node, keyword).name;
      let (start, end) = lines(&node);
      found.push(Scope { entity: entity.clone(), member: None, overrides: false, start, end });
      // Every member container (class_body, enum_body, extension_body) holds class_member nodes.
      let members = node.field("body").into_iter().flat_map(|body| body.children().filter(|c| c.kind() == "class_member").collect::<Vec<_>>());
      for member in members {
        let (start, end) = lines(&member);
        found.push(Scope {
          entity: entity.clone(),
          member: member_name(&member),
          overrides: is_override(&member),
          start,
          end,
        });
      }
    } else if kind == "function_declaration" {
      // A top-level function, its annotations and body included.
      let name = node.field("signature").and_then(|s| s.field("name")).map_or_else(|| "?".to_string(), |n| n.text().to_string());
      let (start, end) = lines(&node);
      found.push(Scope { entity: name, member: None, overrides: false, start, end });
    } else if kind == "top_level_variable_declaration" {
      // One scope per declared name, each spanning the whole declaration.
      let (start, end) = lines(&node);
      for declarator in node.dfs().filter(|n| matches!(n.kind().as_ref(), "initialized_identifier" | "static_final_declaration") && is_top_level_variable(n)) {
        found.push(Scope { entity: variable_entity(&declarator).name, member: None, overrides: false, start, end });
      }
    }
  }
  found
}

/// The name a class member is called by: a method's, getter's or setter's name, or a field's first declared name. None for constructors, which are called by the class's name, and for anything else.
fn member_name(member: &DartNode<'_>) -> Option<String> {
  // The first named declaration in pre-order is the member's own, ahead of anything in its body.
  member.dfs().find_map(|n| match n.kind().as_ref() {
    "function_signature" | "getter_signature" | "setter_signature" => Some(n.field("name").map(|name| name.text().to_string())),
    "constructor_signature" | "factory_constructor_signature" | "redirecting_factory_constructor_signature" | "constant_constructor_signature" => Some(None),
    "initialized_identifier" | "static_final_declaration" => {
      let name: String = n.text().chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect();
      Some((!name.is_empty()).then_some(name))
    }
    _ => None,
  })?
}

/// Whether a class member carries `@override`, which sits either directly in the class_member or in its method_declaration.
fn is_override(member: &DartNode<'_>) -> bool {
  member
    .children()
    .flat_map(|c| std::iter::once(c.clone()).chain(c.children().collect::<Vec<_>>()))
    .any(|n| n.kind() == "annotation" && n.text().trim() == "@override")
}

/// The 1-based lines, ascending and without duplicates, where a pattern matches outside the file's directives. Used to find where an importer references an entity by name: a `show Foo` combinator names Foo without using it.
pub fn references(root: &DartRoot, pattern: &Pattern) -> Vec<usize> {
  let lines: std::collections::BTreeSet<usize> = root
    .root()
    .find_all(pattern)
    .filter(|m| {
      !m.get_node()
        .ancestors()
        .any(|a| a.kind() == "import_or_export")
    })
    .map(|m| m.get_node().start_pos().line() + 1)
    .collect();
  lines.into_iter().collect()
}

/// One pattern match, shaped like ast-grep's JSON output fields the bash
/// version read: the 1-based line, the match's first source line with one
/// leading space removed (jq's `ltrimstr(" ")`), and the full source lines.
pub struct Hit {
  pub line: usize,
  pub text: String,
  pub lines: String,
  /// Text of the requested metavariable, when one was asked for.
  pub capture: Option<String>,
}

pub fn find_all(root: &DartRoot, pattern: &Pattern, capture: Option<&str>) -> Vec<Hit> {
  root
    .root()
    .find_all(pattern)
    .map(|m| {
      let ctx = m.display_context(0, 0);
      let lines = format!("{}{}{}", ctx.leading, ctx.matched, ctx.trailing);
      let first = first_line(&lines);
      let text = first.strip_prefix(' ').unwrap_or(first).to_string();
      let capture = capture.and_then(|var| m.get_env().get_match(var).map(|n| n.text().to_string()));
      Hit {
        line: m.get_node().start_pos().line() + 1,
        text,
        lines,
        capture,
      }
    })
    .collect()
}
