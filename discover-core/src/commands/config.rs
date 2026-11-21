use crate::cli::{Language, detect_lang};
use std::path::Path;
use std::process::ExitCode;

/// Writes a commented `./discover.conf` for the current directory, naming the detected language and every language's default entry points. An existing file is left alone.
pub fn write_config(langs: &[Language]) -> ExitCode {
  // Refuses to overwrite, because the file holds the user's choices.
  if Path::new("discover.conf").exists() {
    eprintln!("error: discover.conf already exists");
    return ExitCode::FAILURE;
  }
  // Reads the directory the file is keyed by.
  let cwd = match crate::project::current_dir() {
    Ok(cwd) => cwd.to_string_lossy().into_owned(),
    Err(e) => {
      eprintln!("error: {e}");
      return ExitCode::FAILURE;
    }
  };
  // Lists one comment line per language.
  let defaults: Vec<String> = langs.iter().map(|lang| format!("#   {}: {}", lang.name, lang.entry_defaults)).collect();
  // Leaves `lang` commented out when nothing identifies the project.
  let lang = match detect_lang(langs, &[]) {
    Some(lang) => format!("  lang: {lang}"),
    None => "  # lang: <language>".to_string(),
  };
  // Quotes the key as JSON, which is also valid YAML, so any path is safe.
  let key = serde_json::to_string(&cwd).expect("a string serializes");
  let text = format!(
    "# discover configuration: one block per project directory. JSON is also accepted.\n#\n# lang     Which navigator to run. Omit it to detect the language from the project.\n# entries  Repo-relative files where the program starts, the roots of liveness.\n#          Listing any replaces the defaults below completely, so list every entry point.\n#          DISCOVER_ENTRY=\"<file> ...\" overrides this list for a single run.\n#\n# Default entry points, used when `entries` is not set:\n{}\n{key}:\n{lang}\n  # entries:\n  #   - path/to/entry\n",
    defaults.join("\n")
  );
  // Writes the file and reports the outcome.
  match std::fs::write("discover.conf", text) {
    Ok(()) => {
      println!("wrote discover.conf");
      ExitCode::SUCCESS
    }
    Err(e) => {
      eprintln!("error: cannot write discover.conf: {e}");
      ExitCode::FAILURE
    }
  }
}
