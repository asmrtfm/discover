//! Ruby's side of the import graph: parsed facts per file, each file's links (its resolved require-style specs, its superclasses and mixins, and, under Zeitwerk, the constants it references), live files (reachability from the entry points over those links) and the importers index. Caching, dead files and trace come from discover-core.

use crate::project::Project;
use crate::rb::{self, Ancestor, Directive, Facts};
use discover_core::cache::{Caches, Fnv};
use discover_core::graph::{EntityUse, Importers, Lang, Row};
use discover_core::memo::Memo;
use discover_core::par::par_map;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

pub type Graph = discover_core::graph::Graph<RbLang>;

pub struct RbLang {
  pub project: Project,
  /// Parsed facts per canonical path, shared across threads.
  facts: Memo<PathBuf, Arc<Facts>>,
  /// Links per canonical path, resolved once each.
  links: Memo<PathBuf, Arc<Vec<Link>>>,
  /// Names each file makes available through its transitive links, computed once each.
  namespaces: Memo<PathBuf, Arc<BTreeSet<String>>>,
  /// Qualified class or module name → the graph files that declare it, built on first use.
  declarations: OnceLock<HashMap<String, Vec<PathBuf>>>,
}

/// One link from a file to another file in the project.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Link {
  pub directive: Directive,
  /// The path or constant as written.
  pub uri: String,
  /// The target's canonical path.
  pub target: PathBuf,
}

/// The graph over a project, its caches stamped with the file listing, the entry points and the configuration that decides resolution.
pub fn open(project: Project) -> Graph {
  let fingerprint = fingerprint(&project);
  let caches = Caches::new(project.cache_dir.clone(), "discover_rs_ruby_", fingerprint);
  Graph::new(RbLang { project, facts: Memo::default(), links: Memo::default(), namespaces: Memo::default(), declarations: OnceLock::new() }, caches)
}

impl RbLang {
  /// Facts for a file, parsed on first use. Unreadable files have none.
  pub fn facts(&self, path: &Path) -> Arc<Facts> {
    self.facts.get(path, || {
      Arc::new(fs::read_to_string(path).map(|src| rb::extract(&rb::parse_file(path, &src))).unwrap_or_default())
    })
  }

  /// The files a file's require, require_relative, load and autoload calls resolve to, in source order.
  fn spec_targets(&self, path: &Path) -> Vec<(Directive, String, PathBuf)> {
    let rel = self.project.rel(path);
    self.facts(path).specs.iter().filter_map(|s| Some((s.directive, s.text.clone(), self.project.existing(&self.project.resolve_spec(s, &rel)?)?))).collect()
  }

  /// Qualified class or module name → the graph files that declare it.
  fn declarations(&self) -> &HashMap<String, Vec<PathBuf>> {
    self.declarations.get_or_init(|| {
      let mut found: HashMap<String, Vec<PathBuf>> = HashMap::new();
      for file in self.project.graph_files() {
        for e in self.facts(&file).entities.iter().filter(|e| matches!(e.kind, "class" | "module")) {
          let files = found.entry(e.qualified.clone()).or_default();
          if !files.contains(&file) {
            files.push(file.clone());
          }
        }
      }
      found
    })
  }

  /// The file that defines a superclass or mixin: where Zeitwerk loads it from; else the nearest file in the require graph below this one (this one included) that declares a class or module by that name, looked up in the ancestor's namespaces like Ruby does; else, when exactly one file in the project declares it, that file (code loaded by some other file's requires, or a Rakefile glob).
  fn resolve_ancestor(&self, path: &Path, ancestor: &Ancestor) -> Option<PathBuf> {
    if let Some(rel) = self.project.resolve_constant(&ancestor.text, &ancestor.nesting) {
      return self.project.existing(&rel);
    }
    // The qualified names the constant can mean, innermost namespace first.
    let names: Vec<String> = match ancestor.text.strip_prefix("::") {
      Some(top) => vec![top.to_string()],
      None => ancestor.nesting.iter().rev().map(|scope| format!("{scope}::{}", ancestor.text)).chain([ancestor.text.clone()]).collect(),
    };
    let declares = |file: &Path| self.facts(file).entities.iter().any(|e| matches!(e.kind, "class" | "module") && names.contains(&e.qualified));
    let mut visited: HashSet<PathBuf> = HashSet::from([path.to_path_buf()]);
    let mut queue = VecDeque::from([path.to_path_buf()]);
    while let Some(file) = queue.pop_front() {
      if declares(&file) {
        return Some(file);
      }
      queue.extend(self.spec_targets(&file).into_iter().map(|(_, _, t)| t).filter(|t| visited.insert(t.clone())));
    }
    // The first name, innermost namespace first, that a single file declares; a name declared in several files (a reopened class) is ambiguous.
    names.iter().find_map(|name| match self.declarations().get(name).map(Vec::as_slice) {
      Some([only]) => Some(only.clone()),
      _ => None,
    })
  }

  /// The files a file links to, in source order without repeats and never itself: its require-style specs, then its superclasses and mixins, then the other autoloaded constants it references.
  pub fn links(&self, path: &Path) -> Arc<Vec<Link>> {
    self.links.get(path, || {
      let facts = self.facts(path);
      let specs = self.spec_targets(path);
      let ancestors: Vec<(Directive, String, PathBuf)> = facts.ancestors.iter().filter_map(|a| Some((a.directive, a.text.clone(), self.resolve_ancestor(path, a)?))).collect();
      // A constant named as an ancestor is linked by its ancestor directive alone.
      let constants = facts
        .constants
        .iter()
        .filter(|c| !facts.ancestors.iter().any(|a| a.text == c.text && a.nesting == c.nesting))
        .filter_map(|c| Some((Directive::Constant, c.text.clone(), self.project.existing(&self.project.resolve_constant(&c.text, &c.nesting)?)?)));
      let mut seen = HashSet::new();
      Arc::new(
        specs
          .into_iter()
          .chain(ancestors)
          .chain(constants)
          .filter(|(_, _, target)| target != path)
          .map(|(directive, uri, target)| Link { directive, uri, target })
          .filter(|l| seen.insert(l.clone()))
          .collect(),
      )
    })
  }

  /// The names a file makes available to its users: its own entities and, following its transitive links (requires, loads, autoloads, superclasses and mixins), those of every file it loads or inherits from.
  pub(crate) fn namespace(&self, path: &Path) -> Arc<BTreeSet<String>> {
    self.namespaces.get(path, || {
      let mut names = BTreeSet::new();
      let mut visited = HashSet::new();
      let mut stack = vec![path.to_path_buf()];
      while let Some(file) = stack.pop() {
        // Guards against require and inheritance cycles.
        if !visited.insert(file.clone()) {
          continue;
        }
        names.extend(self.facts(&file).entity_names());
        stack.extend(self.links(&file).iter().filter(|l| l.directive.is_transitive()).map(|l| l.target.clone()));
      }
      Arc::new(names)
    })
  }
}

impl Lang for RbLang {
  // Names the command in usage messages.
  const NAME: &'static str = "discover --lang=ruby";
  // Ruby's trace replaces core's.
  const OVERRIDES: &'static [&'static str] = &["trace"];

  fn root(&self) -> &Path {
    &self.project.root
  }

  fn entries(&self) -> &HashSet<String> {
    &self.project.entry_set
  }

  fn sources(&self) -> Vec<String> {
    self.project.relevant_files().iter().map(|f| self.project.rel(f)).collect()
  }

  fn entity_names(&self, path: &Path) -> Vec<String> {
    self.facts(path).entity_names()
  }

  /// Requires, loads, autoloads, superclasses and mixins pass the target's entities on to the linking file's users; a plain constant reference doesn't.
  fn is_transitive(directive: &str) -> bool {
    directive.split(',').any(|d| Directive::named(d).is_some_and(Directive::is_transitive))
  }

  /// Ruby files and the entry points (extensionless executables); anything else is an asset.
  fn is_source(&self, path: &Path) -> bool {
    rb::is_ruby(&path.to_string_lossy()) || self.project.entry_set.contains(path.to_string_lossy().as_ref())
  }

  // ─────────────────────────────────────────────────────────
  // live files — breadth-first reachability from the entry
  // points over every link
  // ─────────────────────────────────────────────────────────

  fn compute_live(&self, _caches: &Caches) -> Result<Vec<String>, String> {
    if self.project.entries.is_empty() {
      return Err("no entry point found (list them under entries in discover.conf, see: discover config)".into());
    }

    let mut frontier: Vec<PathBuf> = self.project.entries.iter().filter_map(|e| self.project.existing(e)).collect();
    let mut live: HashSet<PathBuf> = frontier.iter().cloned().collect();
    // Level by level so each frontier is parsed in parallel.
    while !frontier.is_empty() {
      let found: Vec<Vec<PathBuf>> = par_map(&frontier, |current| self.links(current).iter().map(|l| l.target.clone()).collect());
      frontier = found.into_iter().flatten().filter(|p| live.insert(p.clone())).collect();
    }

    let mut lines: Vec<String> = live.iter().map(|p| self.project.rel(p)).collect();
    lines.sort();
    Ok(lines)
  }

  // ─────────────────────────────────────────────────────────
  // importers — reverse index: target → files that require,
  // load, reference or inherit from it, with the target's
  // entities each one uses itself. What their downstream
  // users use is worked out by trace
  // ─────────────────────────────────────────────────────────

  fn compute_importers(&self, _caches: &Caches) -> Result<Importers, String> {
    let files = self.project.graph_files();

    let edges: Vec<Vec<(String, Row)>> = par_map(&files, |file| {
      let facts = self.facts(file);
      let file_rel = self.project.rel(file);
      self
        .links(file)
        .iter()
        .map(|link| {
          let entities_used = used_entities(&facts, &self.facts(&link.target).entity_names());
          let row = Row { file: file_rel.clone(), directive: link.directive.as_str().to_string(), uri: link.uri.clone(), entities_used };
          (self.project.rel(&link.target), row)
        })
        .collect()
    });

    // Grouped by target; rows keep file order within a target.
    let mut index = Importers::new();
    for (target, row) in edges.into_iter().flatten() {
      index.entry(target).or_default().push(row);
    }
    // One row per (importer, target), like the JVM navigators: a file that both requires and includes a target is listed once.
    for rows in index.values_mut() {
      *rows = merge_rows(std::mem::take(rows));
    }
    Ok(index)
  }
}

/// One row per importing file, in first-seen order: its directives joined by `,` and its paths or constants by `, `, each without repeats in first-seen order, and the lines of each entity it uses merged.
pub(crate) fn merge_rows(rows: Vec<Row>) -> Vec<Row> {
  let mut merged: Vec<Row> = Vec::new();
  for row in rows {
    let Some(kept) = merged.iter_mut().find(|r| r.file == row.file) else {
      merged.push(row);
      continue;
    };
    // Appends a part to a joined list unless it is already there.
    let add = |joined: &mut String, part: &str, sep: &str| {
      if !joined.split(sep).any(|p| p == part) {
        joined.push_str(sep);
        joined.push_str(part);
      }
    };
    for directive in row.directive.split(',') {
      add(&mut kept.directive, directive, ",");
    }
    add(&mut kept.uri, &row.uri, ", ");
    kept.entities_used = merge_uses(std::mem::take(&mut kept.entities_used), row.entities_used);
  }
  merged
}

/// Two lists of entity uses as one, by name, with the lines of each merged.
pub(crate) fn merge_uses(a: Vec<EntityUse>, b: Vec<EntityUse>) -> Vec<EntityUse> {
  let mut by_name: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
  for u in a.into_iter().chain(b) {
    by_name.entry(u.name).or_default().extend(u.lines);
  }
  EntityUse::from_map(by_name)
}

/// The target's entities (`own`) a user references, each with the user's lines where its name appears.
pub(crate) fn used_entities(user: &Facts, own: &[String]) -> Vec<EntityUse> {
  let used: BTreeMap<String, BTreeSet<usize>> = own.iter().filter_map(|n| user.idents.get(n).map(|lines| (n.clone(), lines.clone()))).collect();
  EntityUse::from_map(used)
}

/// FNV-1a over the graph's files, the entry points, the configuration files that decide resolution (config/application.rb, the inflections) and the build. Changes when a file is added or removed; content changes that add or drop links need --rebuild-cache.
fn fingerprint(project: &Project) -> String {
  let mut hash = Fnv::default();
  for file in project.graph_files() {
    hash.feed(file.as_os_str().as_encoded_bytes());
  }
  for entry in &project.entries {
    hash.feed(entry.as_bytes());
  }
  for config in &project.config_files {
    hash.feed(&fs::read(config).unwrap_or_default());
  }
  // The build (version and build time), so caches written by an older binary are rebuilt instead of read with its rules.
  hash.feed(discover_core::cli::VERSION.as_bytes());
  hash.hex()
}
