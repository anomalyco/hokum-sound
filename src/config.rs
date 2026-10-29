//! `config.kdl`: Hokum Sound's settings, at `$XDG_CONFIG_HOME/hokum-sound/config.kdl`
//! (`~/.config/hokum-sound/config.kdl`). Each Hokum tool keeps its own folder named after its
//! package (`hokum`, `hokum-sound`), so tools never share or clobber a file. Same vocabulary as
//! Hokum's config:
//!
//! ```kdl
//! behaviour {
//!     appearance "system"     // system, light or dark
//!     check-for-updates #true // fetch the release manifest at start and hourly
//! }
//! ```
//!
//! A missing file means the defaults; a broken one keeps the defaults for what it couldn't
//! read and reports problems (`sound check`).

use std::path::PathBuf;

use kdl::{KdlDocument, KdlNode, KdlValue};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub appearance: Appearance,
    pub check_for_updates: bool,
    /// What couldn't be read, in plain words.
    pub problems: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config { appearance: Appearance::System, check_for_updates: true, problems: Vec::new() }
    }
}

fn base(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(fallback)))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hokum-sound")
}

/// `~/.config/hokum-sound`.
pub fn dir() -> PathBuf {
    base("XDG_CONFIG_HOME", ".config")
}

/// `~/.local/state/hokum-sound`: things the app remembers, not settings.
pub fn state_dir() -> PathBuf {
    base("XDG_STATE_HOME", ".local/state")
}

pub fn path() -> PathBuf {
    dir().join("config.kdl")
}

pub fn load() -> Config {
    match std::fs::read_to_string(path()) {
        Ok(src) => parse(&src),
        Err(_) => Config::default(),
    }
}

fn flag(node: &KdlNode, problems: &mut Vec<String>) -> Option<bool> {
    match node.entries().first().map(|e| e.value()) {
        Some(KdlValue::Bool(b)) => Some(*b),
        _ => {
            problems.push(format!("`{}` takes #true or #false", node.name().value()));
            None
        }
    }
}

pub fn parse(src: &str) -> Config {
    let mut config = Config::default();
    let doc: KdlDocument = match src.parse() {
        Ok(d) => d,
        Err(e) => {
            config.problems.push(format!("not valid KDL: {e}"));
            return config;
        }
    };
    for node in doc.nodes() {
        match node.name().value() {
            "behaviour" => {
                for child in node.children().map(|c| c.nodes()).unwrap_or_default() {
                    match child.name().value() {
                        "appearance" => match child.entries().first().map(|e| e.value()) {
                            Some(KdlValue::String(s)) if s == "system" => config.appearance = Appearance::System,
                            Some(KdlValue::String(s)) if s == "light" => config.appearance = Appearance::Light,
                            Some(KdlValue::String(s)) if s == "dark" => config.appearance = Appearance::Dark,
                            _ => config.problems.push("`appearance` is \"system\", \"light\" or \"dark\"".into()),
                        },
                        "check-for-updates" => {
                            if let Some(b) = flag(child, &mut config.problems) {
                                config.check_for_updates = b;
                            }
                        }
                        other => config.problems.push(format!("unknown behaviour `{other}`; use appearance, check-for-updates")),
                    }
                }
            }
            other => config.problems.push(format!("unknown node `{other}`; use behaviour")),
        }
    }
    config
}

/// `sound check`: prints the config's problems; true when there are none.
pub fn check() -> bool {
    let c = load();
    for p in &c.problems {
        eprintln!("{}: {p}", path().display());
    }
    if c.problems.is_empty() {
        println!("{}: OK", path().display());
    }
    c.problems.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_behaviour() {
        let c = parse("behaviour {\n  appearance \"dark\"\n  check-for-updates #false\n}\n");
        assert_eq!(c.appearance, Appearance::Dark);
        assert!(!c.check_for_updates);
        assert!(c.problems.is_empty());
    }

    #[test]
    fn reports_problems_and_keeps_defaults() {
        let c = parse("behaviour { check-for-updates \"no\"; volume 3 }\nnope\n");
        assert!(c.check_for_updates);
        assert_eq!(c.problems.len(), 3);
        assert!(!parse("{{").problems.is_empty());
    }
}
