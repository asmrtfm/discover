//! The project being navigated: root, source files, Zeitwerk autoload roots, path and constant resolution, and entry points. Nothing project-specific is hardcoded; it is all read from the project:
//!   source files — `.rb` and `.rake` files the project hasn't gitignored (tests and vendored code excluded)
//!   autoloading  — only for projects that use Zeitwerk: a Rails app (config/application.rb defines a `Rails::Application`) or a gem whose lib/ calls `Zeitwerk::Loader.for_gem`. Every other project is linked by its require graph alone.
//!   entry points — $DISCOVER_ENTRY or `entries` in discover.conf, else Ruby executables in bin/ and exe/, the gemspec's lib file, Rakefile, config.ru and rake tasks, plus in Rails what the framework loads by convention; else main.rb or app.rb

use crate::rb::{self, Directive, Spec};
use discover_core::project::{self, relative};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The directories directly under app/ that Rails doesn't autoload.
const NOT_AUTOLOADED: [&str; 3] = ["assets", "javascript", "views"];

/// The directories directly under app/ whose classes the framework invokes (routes, Action Cable, view helpers, Action Mailbox), so every file in them is an entry point.
const INVOKED_BY_RAILS: [&str; 4] = ["app/controllers/", "app/channels/", "app/helpers/", "app/mailboxes/"];

/// An autoload root: the repo-relative directory and the subdirectories of it Zeitwerk ignores.
#[derive(Debug)]
struct Root {
  dir: String,
  ignored: Vec<String>,
}

pub struct Project {
  /// The directory the command is run from, canonical.
  pub root: PathBuf,
  pub cache_dir: PathBuf,
  /// Whether the project is a Rails app.
  pub rails: bool,
  /// Constant → the repo-relative file Zeitwerk loads it from; empty without Zeitwerk.
  constants: BTreeMap<String, String>,
  /// Every Ruby file the project keeps, repo-relative and sorted.
  listing: Vec<String>,
  /// Files whose contents change how paths and constants resolve, for the cache fingerprint.
  pub config_files: Vec<PathBuf>,
  /// Repo-relative entry points, the roots of liveness.
  pub entries: Vec<String>,
  pub entry_set: HashSet<String>,
}

impl Project {
  /// The project in the directory the command is run from, which must hold its Gemfile, gems.rb, a gemspec, Rakefile or config.ru.
  pub fn discover() -> Result<Self, String> {
    let root = project::current_dir()?;
    let has_gemspec = fs::read_dir(&root).into_iter().flatten().flatten().any(|e| e.file_name().to_string_lossy().ends_with(".gemspec"));
    if !has_gemspec {
      project::require_manifest(&root, &["Gemfile", "gems.rb", "Rakefile", "config.ru"])?;
    }
    let cache_dir = project::cache_dir(&root)?;
    let listing = listing(&root);
    let application = root.join("config/application.rb");
    let inflections = root.join("config/initializers/inflections.rb");
    let rails = fs::read_to_string(&application).is_ok_and(|t| t.contains("Rails::Application"));
    let mut project = Project {
      root,
      cache_dir,
      rails,
      constants: BTreeMap::new(),
      listing,
      config_files: vec![application, inflections],
      entries: Vec::new(),
      entry_set: HashSet::new(),
    };
    project.map_constants();
    project.detect_entries()?;
    Ok(project)
  }

  /// Repo-relative form of a canonical path (`realpath --relative-to`).
  pub fn rel(&self, path: &Path) -> String {
    relative(&self.root, path)
  }

  pub fn to_abs(&self, path: &str) -> PathBuf {
    project::to_abs(&self.root, path)
  }

  /// Canonical path of a repo-relative file, if it exists.
  pub fn existing(&self, rel: &str) -> Option<PathBuf> {
    project::existing(&self.root, rel)
  }

  /// Whether the project autoloads constants with Zeitwerk.
  pub fn autoloads(&self) -> bool {
    !self.constants.is_empty()
  }

  /// The source files under analysis, canonical and sorted: `.rb` and `.rake` files, without tests and vendored code. Templates, configs and executables are reached through the entry points instead.
  pub fn relevant_files(&self) -> Vec<PathBuf> {
    self
      .listing
      .iter()
      .filter(|f| (f.ends_with(".rb") || f.ends_with(".rake")) && !is_excluded(f))
      .filter_map(|f| self.existing(f))
      .collect()
  }

  /// Files whose links form the graph: the sources plus the entry points, which may not be sources (templates, executables, an entry under a test directory). Without them their edges would be missing from the importers index and everything below them would trace as dead.
  pub fn graph_files(&self) -> Vec<PathBuf> {
    let mut files = self.relevant_files();
    files.extend(self.entries.iter().filter_map(|e| self.existing(e)));
    files.sort();
    files.dedup();
    files
  }

  /// Resolves a spec found in `from_file` (repo-relative) by its directive's rules.
  pub fn resolve_spec(&self, spec: &Spec, from_file: &str) -> Option<String> {
    self.resolve_path(spec.directive, &spec.text, spec.relative, from_file)
  }

  /// Resolves a path argument to a repo-relative file:
  ///   relative (require_relative, `__dir__` forms) — against the directory of `from_file`
  ///   `/x`                                         — as given
  ///   `./x`, `../x`                                — against the project root, Ruby's working directory
  ///   anything else                                — on the load path: lib/ (load tries the root first)
  /// load takes the file name as given; the others add `.rb` unless it is there. Nothing for gems and the standard library, which aren't in the project.
  pub fn resolve_path(&self, directive: Directive, text: &str, relative: bool, from_file: &str) -> Option<String> {
    if text.is_empty() {
      return None;
    }
    let bases: Vec<PathBuf> = if relative {
      if from_file.is_empty() {
        return None;
      }
      vec![project::importing_dir(&self.root, from_file)?.join(text)]
    } else if text.starts_with('/') {
      vec![PathBuf::from(text)]
    } else if text.starts_with("./") || text.starts_with("../") {
      vec![self.root.join(text)]
    } else if directive == Directive::Load {
      vec![self.root.join(text), self.root.join("lib").join(text)]
    } else {
      vec![self.root.join("lib").join(text)]
    };
    bases
      .into_iter()
      .map(|base| if directive == Directive::Load || text.ends_with(".rb") { base } else { PathBuf::from(format!("{}.rb", base.display())) })
      .find(|p| p.is_file())
      .and_then(|p| fs::canonicalize(p).ok())
      .map(|p| self.rel(&p))
  }

  /// The repo-relative file Zeitwerk loads a referenced constant from. Like Ruby, the first segment is looked up in each enclosing namespace, innermost first, then at the top level (`::X` only there); a reference to a constant defined inside an autoloaded file (`Foo::Bar` in foo.rb) resolves to that file.
  pub fn resolve_constant(&self, text: &str, nesting: &[String]) -> Option<String> {
    if self.constants.is_empty() {
      return None;
    }
    let (scopes, path): (Vec<&str>, &str) = match text.strip_prefix("::") {
      Some(path) => (vec![""], path),
      None => (nesting.iter().rev().map(String::as_str).chain([""]).collect(), text),
    };
    let segments: Vec<&str> = path.split("::").collect();
    for scope in scopes {
      for len in (1..=segments.len()).rev() {
        let tail = segments[..len].join("::");
        let name = if scope.is_empty() { tail } else { format!("{scope}::{tail}") };
        if let Some(file) = self.constants.get(&name) {
          return Some(file.clone());
        }
      }
    }
    None
  }

  /// Zeitwerk's autoload roots: in Rails, each directory under app/ that isn't assets, javascript or views, each app/*/concerns, lib/ with `config.autoload_lib`, and directories added to autoload_paths or eager_load_paths; in a gem, lib/ when a file directly in it calls `Zeitwerk::Loader.for_gem`.
  fn autoload_roots(&self) -> Vec<Root> {
    let mut roots = Vec::new();
    let subdirs = |dir: &Path| -> Vec<String> {
      let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
      // Sorted because directory order varies and the first root to define a constant wins.
      names.sort();
      names
    };
    if self.rails {
      for name in subdirs(&self.root.join("app")).into_iter().filter(|n| !NOT_AUTOLOADED.contains(&n.as_str())) {
        let dir = format!("app/{name}");
        let concerns = format!("{dir}/concerns");
        roots.push(Root { dir, ignored: Vec::new() });
        if self.root.join(&concerns).is_dir() {
          roots.push(Root { dir: concerns, ignored: Vec::new() });
        }
      }
      let config = fs::read_to_string(self.root.join("config/application.rb")).map(|src| rb::rails_config(&rb::parse(&src))).unwrap_or_default();
      if config.autoload_lib {
        roots.push(Root { dir: "lib".to_string(), ignored: config.lib_ignored });
      }
      roots.extend(config.extra_roots.into_iter().map(|dir| Root { dir: dir.trim_end_matches('/').to_string(), ignored: Vec::new() }));
    } else {
      let for_gem = fs::read_dir(self.root.join("lib"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".rb"))
        .any(|e| fs::read_to_string(e.path()).is_ok_and(|t| t.contains("Zeitwerk::Loader.for_gem")));
      if for_gem {
        roots.push(Root { dir: "lib".to_string(), ignored: Vec::new() });
      }
    }
    roots
  }

  /// Fills the constant map from the autoload roots. A file belongs to the deepest root containing it, so app/models/concerns/x.rb is `X`, not `Concerns::X`; the first root to define a constant keeps it.
  fn map_constants(&mut self) {
    let roots = self.autoload_roots();
    if roots.is_empty() {
      return;
    }
    let acronyms = fs::read_to_string(self.root.join("config/initializers/inflections.rb")).map(|src| rb::acronyms(&rb::parse(&src))).unwrap_or_default();
    for file in self.listing.iter().filter(|f| f.ends_with(".rb")) {
      // The deepest root this file sits under, and the path below it.
      let Some((root, below)) = roots
        .iter()
        .filter_map(|r| file.strip_prefix(&format!("{}/", r.dir)).map(|below| (r, below)))
        .max_by_key(|(r, _)| r.dir.len())
      else {
        continue;
      };
      if root.ignored.iter().any(|i| below.starts_with(&format!("{i}/")) || below == format!("{i}.rb")) {
        continue;
      }
      if let Some(constant) = rb::constant_for(below, &acronyms) {
        self.constants.entry(constant).or_insert_with(|| file.clone());
      }
    }
  }

  fn add_entry(&mut self, rel: String) {
    if !rel.is_empty() && self.entry_set.insert(rel.clone()) {
      self.entries.push(rel);
    }
  }

  /// Entry points, in order of precedence:
  ///   custom entries (DISCOVER_ENTRY or discover.conf), used instead of everything below
  ///   Ruby executables in bin/ and exe/, lib/<name>.rb for each <name>.gemspec, Rakefile, config.ru and lib/tasks/**/*.rake
  ///   in Rails, also what the framework loads or invokes by convention: everything under config/ and db/, controllers, channels, helpers and mailboxes, and every ERB and Jbuilder template under app/
  ///   otherwise the first of main.rb, app.rb, lib/main.rb, lib/app.rb that exists
  fn detect_entries(&mut self) -> Result<(), String> {
    if let Some(listed) = project::custom_entries(&self.root)? {
      listed.into_iter().for_each(|rel| self.add_entry(rel));
      return Ok(());
    }

    for dir in ["bin", "exe"] {
      let mut files: Vec<PathBuf> = fs::read_dir(self.root.join(dir)).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
      files.sort();
      for file in files.into_iter().filter(|f| is_ruby_executable(f)) {
        self.add_entry(format!("{dir}/{}", file.file_name().unwrap_or_default().to_string_lossy()));
      }
    }
    let mut gemspecs: Vec<String> = fs::read_dir(&self.root)
      .into_iter()
      .flatten()
      .flatten()
      .filter_map(|e| e.file_name().to_string_lossy().strip_suffix(".gemspec").map(str::to_string))
      .collect();
    gemspecs.sort();
    for name in gemspecs {
      let lib = format!("lib/{name}.rb");
      if self.root.join(&lib).is_file() {
        self.add_entry(lib);
      }
    }
    for file in ["Rakefile", "config.ru"] {
      if self.root.join(file).is_file() {
        self.add_entry(file.to_string());
      }
    }
    let conventional: Vec<String> = self
      .listing
      .iter()
      .filter(|f| {
        let task = f.starts_with("lib/tasks/") && f.ends_with(".rake");
        let rails = self.rails
          && ((f.starts_with("config/") || f.starts_with("db/")) && f.ends_with(".rb")
            // Concerns are modules other classes include, so they need a reference to be live.
            || INVOKED_BY_RAILS.iter().any(|d| f.starts_with(d)) && f.ends_with(".rb") && !f.contains("/concerns/")
            || f.starts_with("app/") && (f.ends_with(".erb") || f.ends_with(".jbuilder")));
        task || rails
      })
      .cloned()
      .collect();
    conventional.into_iter().for_each(|f| self.add_entry(f));
    if !self.entries.is_empty() {
      return Ok(());
    }

    if let Some(main) = ["main.rb", "app.rb", "lib/main.rb", "lib/app.rb"].iter().find(|f| self.root.join(f).is_file()) {
      self.add_entry(main.to_string());
    }
    Ok(())
  }
}

/// Every Ruby file the project keeps, repo-relative and sorted: git's view when available, so vendored gems and anything else the project ignores never enter the graph; otherwise a walk that skips installed and generated directories.
fn listing(root: &Path) -> Vec<String> {
  let patterns = ["*.rb", "*.rake", "*.ru", "*.gemspec", "*.jbuilder", "*.erb", "Rakefile", "Gemfile"];
  let mut files = project::git_listing(root, &patterns).unwrap_or_else(|| {
    let mut found = Vec::new();
    project::walk(root, &[".git", ".bundle", "vendor", "node_modules", "tmp", "log", "coverage"], &mut |path, meta| {
      let rel = relative(root, path);
      if meta.is_file() && rb::is_ruby(&rel) {
        found.push(rel);
      }
    });
    found
  });
  files.retain(|f| rb::is_ruby(f));
  files.sort();
  files
}

/// Whether a file in bin/ or exe/ is a Ruby program: a `.rb` file, or a script whose shebang runs ruby.
fn is_ruby_executable(path: &Path) -> bool {
  if path.extension().is_some_and(|e| e == "rb") {
    return true;
  }
  let Ok(bytes) = fs::read(path) else { return false };
  let first = bytes.split(|b| *b == b'\n').next().unwrap_or_default();
  first.starts_with(b"#!") && String::from_utf8_lossy(first).contains("ruby")
}

/// Tests and vendored code: spec/, test/, tests/, features/, vendor/ (at any depth), *_spec.rb and *_test.rb.
pub fn is_excluded(rel: &str) -> bool {
  rel.ends_with("_spec.rb")
    || rel.ends_with("_test.rb")
    || ["spec/", "test/", "tests/", "features/", "vendor/"].iter().any(|d| rel.starts_with(d) || rel.contains(&format!("/{d}")))
}
