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
pub(crate) type DartNode<'r> = Node<'r, StrDoc<Dart>>;

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

  pub fn entity_names(&self) -> Vec<String> {
    self.entities.iter().map(|e| e.name.clone()).collect()
  }
}

/// Declaration node kinds and the keyword each is reported as, in the order
/// the bash version concatenated them (ties on a line keep this order).
pub(crate) const DECLARATION_KINDS: [(&str, &str); 5] = [
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
pub(crate) fn is_top_level_variable(node: &DartNode<'_>) -> bool {
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

pub(crate) fn first_line(text: &str) -> &str {
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

pub(crate) fn declaration_entity(node: &DartNode<'_>, keyword: &'static str) -> Entity {
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

pub(crate) fn variable_entity(node: &DartNode<'_>) -> Entity {
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
