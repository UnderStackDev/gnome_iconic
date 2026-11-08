//! Small INI helpers for index.theme, .desktop and settings files.
//! Edits change one key and keep every other line as it was.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

pub type Sections = HashMap<String, HashMap<String, String>>;

/// Every section with its keys. Later duplicates of a key win, like GLib.
pub fn parse(text: &str) -> Sections {
    let mut out: Sections = HashMap::new();
    let mut section = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if !k.trim().is_empty() {
                out.entry(section.clone()).or_default().insert(k.trim().into(), v.trim().into());
            }
        }
    }
    out
}

/// Reads a file and parses it; a missing or unreadable file is empty.
pub fn read(path: &Path) -> Sections {
    fs::read_to_string(path).map(|t| parse(&t)).unwrap_or_default()
}

pub fn value(path: &Path, section: &str, key: &str) -> Option<String> {
    read(path).get(section)?.get(key).cloned()
}

/// Splits a comma list (Inherits=, Directories=).
pub fn list(value: Option<&String>) -> Vec<String> {
    value
        .map(|v| v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

/// Returns `text` with `key=value` set in `section`, adding either if missing.
pub fn with_value(text: &str, section: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let header = format!("[{section}]");
    let entry = format!("{key}={value}");
    let start = lines.iter().position(|l| l.trim() == header);
    match start {
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(header);
            lines.push(entry);
        }
        Some(start) => {
            let end = lines[start + 1..]
                .iter()
                .position(|l| l.trim().starts_with('[') && l.trim().ends_with(']'))
                .map_or(lines.len(), |i| start + 1 + i);
            let existing = (start + 1..end).find(|&i| {
                lines[i].split_once('=').is_some_and(|(k, _)| k.trim() == key) && !lines[i].trim().starts_with('#')
            });
            match existing {
                Some(i) => lines[i] = entry,
                None => {
                    // Right after the section's last key, before blank lines.
                    let mut at = end;
                    while at > start + 1 && lines[at - 1].trim().is_empty() {
                        at -= 1;
                    }
                    lines.insert(at, entry);
                }
            }
        }
    }
    lines.join("\n") + "\n"
}

pub fn set_value(path: &Path, section: &str, key: &str, value: &str) -> Result<()> {
    let text = fs::read_to_string(path).unwrap_or_default();
    write_atomic(path, with_value(&text, section, key, value).as_bytes())
}

/// Writes through a temp file and a rename. A symlink (dotfiles) is written
/// through to its target, so the link survives.
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = target.parent().context("path has no parent folder")?;
    fs::create_dir_all(dir).with_context(|| format!("Could not create {}", dir.display()))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(data)?;
    tmp.persist(&target).with_context(|| format!("Could not write {}", target.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_existing_and_new_keys() {
        let text = "[Settings]\na=1\n\n[Other]\nb=2\n";
        let out = with_value(text, "Settings", "a", "9");
        assert_eq!(out, "[Settings]\na=9\n\n[Other]\nb=2\n");
        let out = with_value(text, "Settings", "c", "3");
        assert_eq!(out, "[Settings]\na=1\nc=3\n\n[Other]\nb=2\n");
        let out = with_value(text, "New", "k", "v");
        assert!(out.ends_with("b=2\n\n[New]\nk=v\n"));
    }

    #[test]
    fn parses_lists() {
        let s = parse("[Icon Theme]\nInherits=Adwaita, hicolor,,\n");
        assert_eq!(list(s["Icon Theme"].get("Inherits")), vec!["Adwaita", "hicolor"]);
    }
}
