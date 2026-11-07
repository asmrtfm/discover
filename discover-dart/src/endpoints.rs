//! `endpoints`: every file on the call stack of each API request, as `<file> <url>` lines. The requests are found the way malsi's bin/api-urls finds them; each one is attributed to the scope it is written in (see [`dart::scopes`]), and the stack is walked up from there: to the scopes in the same file that reference it, and through the importers index (the data `trace` reads) to the importers that use the scope's entity and, for a method, call that method by name.

use crate::dart::{self, DartRoot, Scope};
use crate::graph::Graph;
use ast_grep_core::Pattern;
use ast_grep_language::Dart;
use discover_core::graph::{Importers, Lang, Row, Trace};
use regex::Regex;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::sync::LazyLock;

/// A line that is commented out, which makes no requests.
static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*//").expect("valid"));
/// A Retrofit client's baseUrl, given in its @RestApi annotation or in the factory below it.
static BASE_URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"baseUrl:\s*["']([^"']*)"#).expect("valid"));
/// A Retrofit endpoint annotation and its path.
static RETROFIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"@(?:GET|POST|PUT|PATCH|DELETE|HEAD)\(\s*["']([^"']*)"#).expect("valid"));
/// A constant definition such as `usersRoot`, which is a root, not a request.
static CONSTANT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bconst\s+\w+\s*=").expect("valid"));
/// A request method called with an absolute URL, up to the end of its scheme; the opening quote is group 1.
static ABSOLUTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\.(?:get|post|put|patch|delete)(?:<[^>]*>)?\(\s*(["'])https?://"#).expect("valid"));

/// The index of the quote that closes a string literal whose body starts at `from`, with each `${...}` interpolation consumed whole so that quotes nested inside it don't end the literal. None when the literal doesn't close on this line.
fn closing_quote(line: &[u8], from: usize, quote: u8) -> Option<usize> {
  let mut i = from;
  while i < line.len() {
    match line[i] {
      // Skips a whole interpolation when it closes; otherwise the `$` is an ordinary character.
      b'$' if line.get(i + 1) == Some(&b'{') => match line[i + 2..].iter().position(|&b| b == b'}') {
        Some(close) => i += close + 3,
        None => i += 1,
      },
      b if b == quote => return Some(i),
      _ => i += 1,
    }
  }
  None
}

/// The length of the API root a literal's body starts with: `/api/`, or `$` or `${` followed by `rootURL`, `postsRoute` or an identifier ending in `Root`. None when it starts with none of them.
fn api_root(body: &[u8]) -> Option<usize> {
  if body.starts_with(b"/api/") {
    return Some(5);
  }
  if body.first() != Some(&b'$') {
    return None;
  }
  // Skips the `$` and the optional `{`.
  let mut at = 1 + usize::from(body.get(1) == Some(&b'{'));
  let ident_len = body[at..].iter().take_while(|b| b.is_ascii_alphanumeric() || **b == b'_').count();
  let ident = &body[at..at + ident_len];
  at += ident_len;
  let is_root = ident == b"rootURL" || ident == b"postsRoute" || (ident.len() > 4 && ident.ends_with(b"Root"));
  is_root.then_some(at)
}

/// The URLs a file requests, with the 1-based line of each, exactly as written (path variables are left unresolved): Retrofit endpoint annotations prefixed with their client's baseUrl, string literals that start with an API root, and absolute http(s) URLs passed straight to a request method.
pub fn url_sites(src: &str) -> Vec<(usize, String)> {
  let mut sites = Vec::new();
  // Each file starts with no Retrofit baseUrl.
  let mut base = String::new();
  for (n, line) in src.lines().enumerate() {
    if COMMENT.is_match(line) {
      continue;
    }
    // A @RestApi annotation starts a new Retrofit client.
    if line.contains("@RestApi(") {
      base.clear();
    }
    if let Some(c) = BASE_URL.captures(line) {
      base = c[1].to_string();
    }
    if let Some(c) = RETROFIT.captures(line) {
      sites.push((n + 1, format!("{base}{}", &c[1])));
    }
    if CONSTANT.is_match(line) {
      continue;
    }
    let bytes = line.as_bytes();
    // Literals that start with an API root, tried at every quote; a match resumes the search after its closing quote.
    let mut i = 0;
    while i < bytes.len() {
      let quote = bytes[i];
      let found = (quote == b'"' || quote == b'\'')
        .then(|| api_root(&bytes[i + 1..]))
        .flatten()
        .and_then(|root| closing_quote(bytes, i + 1 + root, quote));
      match found {
        Some(close) => {
          sites.push((n + 1, line[i + 1..close].to_string()));
          i = close + 1;
        }
        None => i += 1,
      }
    }
    // Absolute URLs passed straight to a request method.
    let mut from = 0;
    while let Some(c) = ABSOLUTE.captures_at(line, from) {
      let whole = c.get(0).expect("group 0");
      let open = c.get(1).expect("group 1").start();
      match closing_quote(bytes, whole.end(), bytes[open]) {
        Some(close) => {
          sites.push((n + 1, line[open + 1..close].to_string()));
          from = close + 1;
        }
        None => from = whole.start() + 1,
      }
    }
  }
  sites
}

/// A parsed file and its scopes.
struct Parsed {
  root: DartRoot,
  scopes: Vec<Scope>,
}

/// A scope on the call stack, in the file it is written in.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Frame {
  file: String,
  scope: Scope,
}

/// Walks call stacks, parsing each file once and building each name's pattern once.
struct Walker<'a> {
  graph: &'a Graph,
  index: &'a Importers,
  trace: Trace<'a>,
  /// Whether dead files are walked too.
  all: bool,
  parsed: HashMap<String, Option<Parsed>>,
  patterns: HashMap<String, Option<Pattern>>,
}

impl Walker<'_> {
  /// Whether a file is walked: every file with `--all`, else only those trace finds reachable from an entry point.
  fn walks(&mut self, file: &str) -> bool {
    self.all || self.trace.reachable(file)
  }

  /// The file's parse, read on first use; None when it can't be read.
  fn parsed(&mut self, file: &str) -> Option<&Parsed> {
    let root = &self.graph.project.root;
    self
      .parsed
      .entry(file.to_string())
      .or_insert_with(|| {
        let src = fs::read_to_string(root.join(file)).ok()?;
        let root = dart::parse(&src);
        let scopes = dart::scopes(&root);
        Some(Parsed { root, scopes })
      })
      .as_ref()
  }

  /// The lines in a file where `name` is referenced, outside its directives.
  fn references(&mut self, file: &str, name: &str) -> Vec<usize> {
    let pattern = self.patterns.entry(name.to_string()).or_insert_with(|| Pattern::try_new(name, Dart).ok());
    let Some(pattern) = pattern.clone() else {
      return Vec::new();
    };
    self.parsed(file).map(|p| dart::references(&p.root, &pattern)).unwrap_or_default()
  }

  /// The innermost scope around each line, as frames in `file`, without duplicates.
  fn frames_at(&mut self, file: &str, lines: &[usize]) -> Vec<Frame> {
    let Some(parsed) = self.parsed(file) else {
      return Vec::new();
    };
    let found: BTreeSet<usize> = lines
      .iter()
      .filter_map(|&line| {
        // The fewest lines wins, so a member beats the entity around it.
        (0..parsed.scopes.len())
          .filter(|&i| parsed.scopes[i].start <= line && line <= parsed.scopes[i].end)
          .min_by_key(|&i| parsed.scopes[i].end - parsed.scopes[i].start)
      })
      .collect();
    found.into_iter().map(|i| Frame { file: file.to_string(), scope: parsed.scopes[i].clone() }).collect()
  }

  /// The rows of files that import `file`, directly or through files that re-export it, each flagged with whether it was reached through a re-export (whose `entities_used` then don't name `file`'s entities).
  fn importer_rows(&self, file: &str) -> Vec<(&Row, bool)> {
    let mut rows = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = vec![(file.to_string(), false)];
    while let Some((target, through_export)) = stack.pop() {
      // Guards against export cycles.
      if !visited.insert(target.clone()) {
        continue;
      }
      for row in self.index.get(&target).into_iter().flatten() {
        let directives: Vec<&str> = row.directive.split(',').collect();
        if directives.contains(&"import") {
          rows.push((row, through_export));
        }
        // A re-exporting file passes the entities on to its own importers.
        if directives.contains(&"export") {
          stack.push((row.file.clone(), true));
        }
      }
    }
    rows
  }

  /// The frames that call `frame`: the scopes in its own file that reference it, then the scopes in each walked importer that use its entity and, unless it is the entity itself, a constructor or an override, call its member by name.
  fn callers(&mut self, frame: &Frame) -> Vec<Frame> {
    let Frame { file, scope } = frame;
    // The name a caller writes: the member's for a method called by name, else the entity's.
    let by_member = scope.member.as_deref().filter(|_| !scope.overrides);
    let name = by_member.unwrap_or(&scope.entity);

    // Callers in the same file, outside the scope itself.
    let local: Vec<usize> = self.references(file, name).into_iter().filter(|l| *l < scope.start || *l > scope.end).collect();
    let mut found = self.frames_at(file, &local);

    // A private entity is visible only to its own library.
    if scope.entity.starts_with('_') {
      return found;
    }
    let rows: Vec<(String, Vec<usize>, bool)> = self
      .importer_rows(file)
      .into_iter()
      .map(|(row, through_export)| {
        let used = row.entities_used.iter().find(|u| u.name == scope.entity).map(|u| u.lines.clone()).unwrap_or_default();
        (row.file.clone(), used, through_export)
      })
      .collect();
    for (importer, used, through_export) in rows {
      if !self.walks(&importer) {
        continue;
      }
      // Trace's entity lines for a direct importer; a re-export's importers are searched for the entity themselves.
      let used = if through_export { self.references(&importer, &scope.entity) } else { used };
      if used.is_empty() {
        continue;
      }
      let lines = match by_member {
        Some(member) => self.references(&importer, member),
        None => used,
      };
      found.extend(self.frames_at(&importer, &lines));
    }
    found
  }

  /// Every walked file on the call stack of a request written at `line` in `file`, the file itself included.
  fn stack(&mut self, file: &str, line: usize) -> BTreeSet<String> {
    let mut files = BTreeSet::from([file.to_string()]);
    let mut visited = HashSet::new();
    let mut work = self.frames_at(file, &[line]);
    while let Some(frame) = work.pop() {
      if !visited.insert(frame.clone()) {
        continue;
      }
      files.insert(frame.file.clone());
      work.extend(self.callers(&frame));
    }
    files
  }
}

/// `endpoints [<url-regex> ...] [--all]`: one `<file> <url>` line per file on the call stack of each request whose URL, as written in the source, any of the regexes matches (every request when none are given), sorted and without duplicates. Only live files are walked unless `all` is set.
pub fn endpoints(graph: &Graph, patterns: &[String], all: bool) -> Result<Vec<String>, String> {
  // Compiles the selectors up front, so a bad one fails before any work is done.
  let filters: Vec<Regex> = patterns.iter().map(|p| Regex::new(p).map_err(|e| format!("invalid regex {p:?}: {e}"))).collect::<Result<_, _>>()?;
  let index = graph.importers()?;
  let mut walker = Walker {
    graph,
    index: &index,
    trace: Trace::new(&index, graph.entries()),
    all,
    parsed: HashMap::new(),
    patterns: HashMap::new(),
  };
  let mut lines = BTreeSet::new();
  for path in graph.project.relevant_files() {
    let file = graph.project.rel(&path);
    let Ok(src) = fs::read_to_string(&path) else {
      continue;
    };
    let sites: Vec<(usize, String)> =
      url_sites(&src).into_iter().filter(|(_, url)| filters.is_empty() || filters.iter().any(|f| f.is_match(url))).collect();
    // Reads the file's liveness only when it requests a selected URL.
    if sites.is_empty() || !walker.walks(&file) {
      continue;
    }
    for (line, url) in sites {
      for caller in walker.stack(&file, line) {
        lines.insert(format!("{caller} {url}"));
      }
    }
  }
  Ok(lines.into_iter().collect())
}
