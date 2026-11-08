//! Removing themes the user installed.

use super::themes::ThemeInfo;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Only the user's own themes, and never the one in use.
pub fn can_remove(themes: &[ThemeInfo], id: &str, current: &str) -> bool {
    id != current && themes.iter().any(|t| t.id == id && t.user)
}

/// Other user themes that link into `id` (Papirus-Dark into Papirus) and
/// would break without it.
pub fn dependents(themes: &[ThemeInfo], id: &str) -> Vec<String> {
    let Some(target) = themes.iter().find(|t| t.id == id).and_then(|t| fs::canonicalize(&t.path).ok()) else {
        return vec![];
    };
    themes
        .iter()
        .filter(|t| t.user && t.id != id && links_into(&t.path, &target))
        .map(|t| t.id.clone())
        .collect()
}

/// Variants link whole size/context folders (16x16/apps), two levels down;
/// looking there is enough and stays fast.
fn links_into(root: &Path, target: &Path) -> bool {
    let Ok(sizes) = fs::read_dir(root) else { return false };
    for size in sizes.flatten().filter(|e| e.path().is_dir()) {
        let Ok(entries) = fs::read_dir(size.path()) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(raw) = fs::read_link(&path) else { continue };
            // Resolved by hand, so a link counts even if its target is gone.
            let resolved = clean(&path.parent().unwrap_or(root).join(raw));
            if resolved.starts_with(target) {
                return true;
            }
        }
    }
    false
}

/// Normalises "." and ".." without touching the disk.
pub fn clean(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

pub fn remove(themes: &[ThemeInfo], id: &str, current: &str) -> Result<()> {
    if !can_remove(themes, id, current) {
        bail!("\"{id}\" can't be removed: it's a system theme or in use");
    }
    let users = dependents(themes, id);
    if !users.is_empty() {
        bail!("{} uses files from {id}; remove it first", users.join(", "));
    }
    let path = &themes.iter().find(|t| t.id == id).expect("checked above").path;
    // A symlinked theme loses only its link, never the folder it points to.
    let result = if path.is_symlink() { fs::remove_file(path) } else { fs::remove_dir_all(path) };
    result.with_context(|| format!("Could not remove {}", path.display()))
}
