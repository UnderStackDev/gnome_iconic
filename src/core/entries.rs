//! The app launchers (.desktop files) GNOME shows, and Iconic's state in them.
//!
//! GNOME reads ~/.local/share/applications first and the first file per
//! desktop id wins, so a copy there with the same id replaces the package's
//! launcher. Copies are marked X-Iconic-Override=true.

use super::{ini, paths};
use gtk::glib;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const MARK: &str = "X-Iconic-Override";

#[derive(Clone, Debug, Default)]
pub struct Entry {
    /// Path below applications/, "/" -> "-", without .desktop.
    pub id: String,
    /// The file that wins.
    pub path: PathBuf,
    pub name: String,
    pub comment: String,
    /// Icon= as written: a name or an absolute path.
    pub icon: String,
    /// NoDisplay or Hidden.
    pub hidden: bool,
    /// OnlyShowIn/NotShowIn keep it off this desktop.
    pub hidden_by_desktop: bool,
    /// Our copy in the user folder.
    pub overridden: bool,
    /// A user launcher edited in place, with a backup.
    pub backed_up: bool,
    /// What Reset goes back to (the package file or the backup).
    pub original: Option<PathBuf>,
}

impl Entry {
    pub fn modified(&self) -> bool {
        self.overridden || self.backed_up
    }
}

pub type Keys = HashMap<String, String>;

/// The [Desktop Entry] group of a file, or None if it has none.
pub fn read_keys(path: &Path) -> Option<Keys> {
    ini::read(path).remove("Desktop Entry")
}

fn is_true(keys: &Keys, key: &str) -> bool {
    keys.get(key).is_some_and(|v| v.eq_ignore_ascii_case("true"))
}

/// Name[fi_FI], Name[fi], ... Name, in the user's language order.
pub fn localized(keys: &Keys, key: &str) -> Option<String> {
    glib::language_names()
        .iter()
        .find_map(|lang| keys.get(&format!("{key}[{lang}]")))
        .or_else(|| keys.get(key))
        .cloned()
}

fn desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().split(':').map(String::from).collect()
}

fn list_matches(list: Option<&String>, desktops: &[String]) -> bool {
    list.is_some_and(|l| l.split(';').any(|d| !d.is_empty() && desktops.iter().any(|x| x == d)))
}

fn collect(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            collect(&p, base, out);
        } else if p.extension().is_some_and(|e| e == "desktop") {
            let rel = p.strip_prefix(base).expect("below base").with_extension("");
            out.push((rel.to_string_lossy().replace('/', "-"), p));
        }
    }
}

/// Every Application launcher, the winning file per id, sorted by name.
pub fn scan() -> Vec<Entry> {
    scan_in(&paths::application_dirs(), &paths::backups())
}

pub fn scan_in(dirs: &[PathBuf], backups: &Path) -> Vec<Entry> {
    let mut files: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut order = vec![];
    for dir in dirs {
        let mut found = vec![];
        collect(dir, dir, &mut found);
        for (id, path) in found {
            let list = files.entry(id.clone()).or_default();
            if list.is_empty() {
                order.push(id);
            }
            list.push(path);
        }
    }
    let desktops = desktops();
    let mut out: Vec<Entry> = order
        .into_iter()
        .filter_map(|id| {
            let candidates = &files[&id];
            let keys = read_keys(&candidates[0])?;
            if keys.get("Type").is_some_and(|t| t != "Application") {
                return None;
            }
            let overridden = is_true(&keys, MARK);
            let backup = backups.join(format!("{id}.desktop"));
            let backed_up = !overridden && backup.exists();
            let original = if overridden {
                candidates.get(1).cloned()
            } else {
                backed_up.then_some(backup)
            };
            Some(Entry {
                name: localized(&keys, "Name").unwrap_or_else(|| id.clone()),
                comment: localized(&keys, "Comment").unwrap_or_default(),
                icon: keys.get("Icon").cloned().unwrap_or_default(),
                hidden: is_true(&keys, "NoDisplay") || is_true(&keys, "Hidden"),
                hidden_by_desktop: (keys.contains_key("OnlyShowIn") && !list_matches(keys.get("OnlyShowIn"), &desktops))
                    || list_matches(keys.get("NotShowIn"), &desktops),
                path: candidates[0].clone(),
                overridden,
                backed_up,
                original,
                id,
            })
        })
        .collect();
    out.sort_by_key(|e| e.name.to_lowercase());
    out
}
