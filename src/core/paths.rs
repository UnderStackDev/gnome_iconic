//! Where things live, per the XDG base directory spec.

use std::env;
use std::path::PathBuf;

pub fn home() -> PathBuf {
    PathBuf::from(env::var("HOME").expect("HOME is not set"))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    match env::var(var) {
        Ok(v) if !v.is_empty() => PathBuf::from(v),
        _ => home().join(fallback),
    }
}

pub fn config_home() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
}

pub fn data_home() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share")
}

/// XDG_DATA_DIRS, without the user's own data home.
pub fn data_dirs() -> Vec<PathBuf> {
    let raw = env::var("XDG_DATA_DIRS").unwrap_or_default();
    let raw = if raw.is_empty() { "/usr/local/share:/usr/share".to_string() } else { raw };
    let mut out: Vec<PathBuf> = Vec::new();
    for part in raw.split(':').filter(|p| !p.is_empty()) {
        let p = PathBuf::from(part);
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Where installed themes go. Flatpak apps can see this folder too.
pub fn user_icons() -> PathBuf {
    data_home().join("icons")
}

/// Icon theme folders in lookup order: the user's own first.
pub fn icon_search_paths() -> Vec<PathBuf> {
    let mut out = vec![user_icons(), home().join(".icons")];
    for dir in data_dirs() {
        let p = dir.join("icons");
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out.push(PathBuf::from("/usr/share/pixmaps"));
    out
}

/// applications/ folders in lookup order: the user's own first.
pub fn application_dirs() -> Vec<PathBuf> {
    let mut out = vec![user_applications()];
    for dir in data_dirs() {
        let p = dir.join("applications");
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

pub fn user_applications() -> PathBuf {
    data_home().join("applications")
}

/// Originals of the user's own launchers, kept before the first edit.
pub fn backups() -> PathBuf {
    data_home().join("iconic/backups")
}

/// Icon files picked for launchers.
pub fn custom_icons() -> PathBuf {
    data_home().join("iconic/icons")
}
