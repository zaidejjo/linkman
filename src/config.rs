//! JSON/JSONC configuration read from `~/.config/linkman/config.{json,jsonc}`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::fsops;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Start the TUI with relative links enabled.
    pub relative: bool,
    /// Start the TUI with force enabled.
    pub force: bool,
    /// Show dotfiles when a pane opens.
    pub show_hidden: bool,
    /// How many operations the TUI remembers for undo.
    pub undo_limit: usize,
    /// Colour palette: black, catppuccin-mocha, tokyo-night, dracula,
    /// gruvbox, everforest or github-light.
    pub theme: String,
    /// Optional starting folder for the SOURCE pane (CLI arguments win).
    pub source: Option<PathBuf>,
    /// Optional starting folder for the DEST pane (CLI arguments win).
    pub dest: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            relative: false,
            force: false,
            show_hidden: false,
            undo_limit: 50,
            theme: "black".to_string(),
            source: None,
            dest: None,
        }
    }
}

impl Config {
    pub const SAMPLE: &'static str = r#"{
  // linkman TUI preferences.
  // Relative links survive moving a whole tree; absolute links survive
  // moving just the link.
  "relative": false,

  // Replace existing entries without asking (the TUI still refuses dirs).
  "force": false,

  // Show dotfiles in both panes on startup.
  "show_hidden": false,

  // Number of operations remembered by undo (1–500).
  "undo_limit": 50,

  // Palette: black (default), catppuccin-mocha, tokyo-night, dracula,
  // gruvbox, everforest or github-light. Ctrl+P can cycle them too.
  "theme": "black",

  // Optional starting folders. CLI arguments always take precedence.
  "source": null,
  "dest": null
}
"#;

    fn sanitized(mut self) -> Self {
        self.undo_limit = self.undo_limit.clamp(1, 500);
        self.theme = self.theme.trim().to_string();
        if self.theme.is_empty() {
            self.theme = "black".to_string();
        }
        self.source = self.source.map(expand);
        self.dest = self.dest.map(expand);
        self
    }
}

fn expand(path: PathBuf) -> PathBuf {
    fsops::expand_path(&path.to_string_lossy())
}

pub fn config_dir() -> Option<PathBuf> {
    if let Some(xdg) = env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(xdg).join("linkman"));
    }
    fsops::home_dir().map(|home| home.join(".config").join("linkman"))
}

/// Where `--init` writes the file when nothing exists yet.
pub fn default_path() -> Option<PathBuf> {
    if let Some(custom) = env::var_os("LINKMAN_CONFIG").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(custom));
    }
    config_dir().map(|dir| dir.join("config.jsonc"))
}

fn candidates() -> Vec<PathBuf> {
    if let Some(custom) = env::var_os("LINKMAN_CONFIG").filter(|value| !value.is_empty()) {
        return vec![PathBuf::from(custom)];
    }
    match config_dir() {
        Some(dir) => vec![dir.join("config.json"), dir.join("config.jsonc")],
        None => Vec::new(),
    }
}

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: Config,
    pub path: Option<PathBuf>,
    pub warning: Option<String>,
}

pub fn load() -> LoadedConfig {
    let mut loaded = LoadedConfig {
        config: Config::default(),
        path: None,
        warning: None,
    };

    for path in candidates() {
        if !path.is_file() {
            continue;
        }
        loaded.path = Some(path.clone());
        match fs::read_to_string(&path) {
            Ok(raw) => match parse(&raw) {
                Ok(config) => loaded.config = config.sanitized(),
                Err(err) => {
                    loaded.warning = Some(format!(
                        "config error in {}: {err}",
                        crate::util::tildify(&path)
                    ));
                }
            },
            Err(err) => {
                loaded.warning = Some(format!(
                    "cannot read {}: {err}",
                    crate::util::tildify(&path)
                ));
            }
        }
        break;
    }

    loaded
}

/// Parse JSON or JSONC (comments and trailing commas are welcome).
pub fn parse(raw: &str) -> Result<Config, String> {
    let cleaned = strip_jsonc(raw.trim_start_matches('\u{feff}'));
    serde_json::from_str::<Config>(&cleaned).map_err(|err| err.to_string())
}

fn strip_jsonc(input: &str) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Normal,
        String,
        Escape,
        LineComment,
        BlockComment,
        BlockStar,
    }

    let mut out = String::with_capacity(input.len());
    let mut state = State::Normal;
    for c in input.chars() {
        match state {
            State::Normal => match c {
                '"' => {
                    state = State::String;
                    out.push(c);
                }
                '/' => state = State::LineComment,
                _ => out.push(c),
            },
            State::String => {
                out.push(c);
                match c {
                    '\\' => state = State::Escape,
                    '"' => state = State::Normal,
                    _ => {}
                }
            }
            State::Escape => {
                out.push(c);
                state = State::String;
            }
            State::LineComment => match c {
                '\n' => {
                    out.push('\n');
                    state = State::Normal;
                }
                _ => out.push(' '),
            },
            State::BlockComment => match c {
                '*' => state = State::BlockStar,
                '\n' => out.push('\n'),
                _ => out.push(' '),
            },
            State::BlockStar => match c {
                '/' => state = State::Normal,
                '*' => out.push(' '),
                '\n' => {
                    out.push('\n');
                    state = State::BlockComment;
                }
                _ => {
                    out.push(' ');
                    state = State::BlockComment;
                }
            },
        }
    }

    // The opening `/` of a comment was consumed as a marker; if the text
    // ended in a comment we simply drop it, which is valid JSONC.
    remove_trailing_commas(&out)
}

fn remove_trailing_commas(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escaped = false;

    for (index, &c) in chars.iter().enumerate() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }
        if c == ',' {
            let mut next = index + 1;
            while next < chars.len() && chars[next].is_whitespace() {
                next += 1;
            }
            if matches!(chars.get(next).copied(), Some(']') | Some('}')) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

pub fn write_sample(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, Config::SAMPLE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jsonc_with_comments_and_trailing_commas() {
        let raw = r#"
        {
          // relative links are handy
          "relative": true,
          "undo_limit": 12,
          /* URL-looking values must survive */
          "source": "~/projects // not a comment",
          "dest": "/tmp/links",
        }
        "#;
        let config = parse(raw).unwrap();
        assert!(config.relative);
        assert_eq!(config.undo_limit, 12);
        assert_eq!(
            config.source.unwrap(),
            PathBuf::from("~/projects // not a comment")
        );
        assert_eq!(config.dest.unwrap(), PathBuf::from("/tmp/links"));
    }

    #[test]
    fn comments_inside_strings_are_kept() {
        let config = parse(r#"{"source": "https://example.com/a", "relative": true}"#).unwrap();
        assert!(config.relative);
        assert_eq!(
            config.source.unwrap(),
            PathBuf::from("https://example.com/a")
        );
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(parse(r#"{"relativ": true}"#).is_err());
    }

    #[test]
    fn undo_limit_is_clamped() {
        let loaded = parse(r#"{"undo_limit": 0}"#).unwrap().sanitized();
        assert_eq!(loaded.undo_limit, 1);
        let loaded = parse(r#"{"undo_limit": 9999}"#).unwrap().sanitized();
        assert_eq!(loaded.undo_limit, 500);
    }

    #[test]
    fn theme_defaults_to_black_and_is_kept_verbatim() {
        assert_eq!(Config::default().theme, "black");
        let config = parse(r#"{"theme": "  Catppuccin Mocha  "}"#)
            .unwrap()
            .sanitized();
        assert_eq!(config.theme, "Catppuccin Mocha");
        let config = parse(r#"{"theme": "   "}"#).unwrap().sanitized();
        assert_eq!(config.theme, "black");
    }

    #[test]
    fn trailing_comma_and_block_comments() {
        let config = parse("{\n \"force\": true, /* yes */\n}").unwrap();
        assert!(config.force);
    }
}
