//! Changing launchers without touching a package's own files.
//! Package launchers get a marked copy in the user folder; launchers the
//! user made are edited in place after a one-time backup.

use super::entries::{Entry, MARK, read_keys};
use super::{ini, paths};
use anyhow::{Context, Result, bail};
use gtk::glib;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct Change {
    /// None = keep.
    pub name: Option<String>,
    pub icon: Option<String>,
    pub hidden: Option<bool>,
}

/// Applies `change` to `entry`; returns the file written.
pub fn apply(entry: &Entry, change: &Change) -> Result<PathBuf> {
    let user_dir = paths::user_applications();
    let target = if entry.overridden {
        entry.path.clone()
    } else if entry.path.starts_with(&user_dir) {
        let backup = paths::backups().join(format!("{}.desktop", entry.id));
        if !backup.exists() {
            ini::write_atomic(&backup, &fs::read(&entry.path)?)?;
        }
        entry.path.clone()
    } else {
        let target = user_dir.join(format!("{}.desktop", entry.id));
        let content = fs::read(&entry.path).with_context(|| format!("Could not read {}", entry.path.display()))?;
        ini::write_atomic(&target, &content)?;
        ini::set_value(&target, "Desktop Entry", MARK, "true")?;
        target
    };

    let set = |k: &str, v: &str| ini::set_value(&target, "Desktop Entry", k, v);
    if let Some(name) = &change.name {
        set("Name", name.replace('\n', " ").trim())?;
        drop_localized_names(&target)?;
    }
    if let Some(icon) = &change.icon {
        set("Icon", icon.trim())?;
    }
    if let Some(hidden) = change.hidden {
        set("NoDisplay", if hidden { "true" } else { "false" })?;
        // Hidden=true means "deleted"; showing the app again has to clear it.
        let deleted = read_keys(&target).and_then(|k| k.get("Hidden").cloned());
        if !hidden && deleted.is_some_and(|v| v.eq_ignore_ascii_case("true")) {
            set("Hidden", "false")?;
        }
    }
    refresh_database()?;
    Ok(target)
}

/// Drops Name[xx]= from [Desktop Entry], so a new name shows in every language.
fn drop_localized_names(path: &Path) -> Result<()> {
    let text = fs::read_to_string(path)?;
    let mut main = false;
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| {
            let t = l.trim();
            if t.starts_with('[') {
                main = t == "[Desktop Entry]";
            }
            !(main && t.starts_with("Name["))
        })
        .collect();
    ini::write_atomic(path, (kept.join("\n") + "\n").as_bytes())
}

/// Undoes Iconic's changes: deletes the copy, or restores the backup.
pub fn reset(entry: &Entry) -> Result<()> {
    if entry.overridden {
        fs::remove_file(&entry.path).with_context(|| format!("Could not remove {}", entry.path.display()))?;
    } else if entry.backed_up {
        let backup = entry.original.as_ref().expect("backed up entries have one");
        ini::write_atomic(&entry.path, &fs::read(backup)?)?;
        fs::remove_file(backup)?;
    }
    refresh_database()
}

/// Copies a picked image to Iconic's icon folder; returns the path for Icon=.
pub fn import_icon(file: &Path, id: &str) -> Result<PathBuf> {
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !matches!(ext.as_str(), "png" | "svg" | "xpm") {
        bail!("Icons must be PNG, SVG or XPM files");
    }
    let data = fs::read(file).with_context(|| format!("Could not read {}", file.display()))?;
    // Content-addressed, so the same picture picked twice is stored once.
    let hash = glib::compute_checksum_for_data(glib::ChecksumType::Sha256, &data).expect("sha256");
    let safe: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' }).collect();
    let target = paths::custom_icons().join(format!("{safe}-{}.{ext}", &hash[..10]));
    if !target.exists() {
        ini::write_atomic(&target, &data)?;
    }
    Ok(target)
}

/// Keeps the MIME cache of the user's launcher folder in step.
fn refresh_database() -> Result<()> {
    let status = Command::new("update-desktop-database")
        .arg("-q")
        .arg(paths::user_applications())
        .status()
        .context("Could not run update-desktop-database")?;
    if !status.success() {
        bail!("update-desktop-database failed");
    }
    Ok(())
}
