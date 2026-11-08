//! Installs icon themes from archives or folders into ~/.local/share/icons.

mod archive;
mod copy;

pub use archive::is_archive;

use super::themes::parse_index;
use super::{paths, removal::clean, settings::valid_id};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Conflict {
    Replace,
    KeepBoth,
    Skip,
}

#[derive(Debug)]
pub struct Found {
    pub id: String,
    pub name: String,
    pub source: PathBuf,
    /// A theme with this id is already in the target folder.
    pub exists: bool,
}

/// A dropped file or folder, unpacked and searched, ready to install.
#[derive(Debug)]
pub struct Prepared {
    pub themes: Vec<Found>,
    /// Plain-language notes: cursor themes, skipped duplicates.
    pub notes: Vec<String>,
    root: PathBuf,
    unpacked: Option<TempDir>,
}

impl Prepared {
    pub fn has_conflicts(&self) -> bool {
        self.themes.iter().any(|t| t.exists)
    }
}

pub fn prepare(path: &Path, target: &Path) -> Result<Prepared> {
    let file = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let meta = fs::metadata(path).with_context(|| format!("{} does not exist", path.display()))?;
    let (root, unpacked) = if meta.is_dir() {
        let root = fs::canonicalize(path)?;
        if clean(&root).starts_with(clean(target)) {
            bail!("{file} is already in the icon folder");
        }
        (root, None)
    } else if is_archive(path) {
        // Unpacked beside the target, on the same disk, so installing can
        // move files instead of copying tens of thousands of them.
        fs::create_dir_all(target)?;
        let tmp = tempfile::Builder::new().prefix(".iconic-unpack-").tempdir_in(target)?;
        archive::unpack(path, tmp.path())?;
        (tmp.path().to_path_buf(), Some(tmp))
    } else {
        bail!("{file} is not a folder or a supported archive (.tar.gz, .tar.xz, .zip, .7z)");
    };

    let mut out = Prepared { themes: vec![], notes: vec![], root: root.clone(), unpacked };
    find_themes(&root, target, &mut out)?;
    if out.themes.is_empty() {
        if out.notes.is_empty() {
            bail!("No icon theme found in {file}");
        }
        bail!("{}", out.notes.join("; "));
    }
    Ok(out)
}

fn find_themes(dir: &Path, target: &Path, out: &mut Prepared) -> Result<()> {
    let index = dir.join("index.theme");
    // A symlinked index.theme could point anywhere; only real files count.
    if index.is_file() && !index.is_symlink() {
        if let Some(parsed) = parse_index(&index) {
            let id = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
            if parsed.directories.is_empty() {
                out.notes.push(format!("{id} is a cursor theme, not an icon theme"));
            } else if !valid_id(&id) {
                out.notes.push(format!("{id} has an unusable folder name"));
            } else if !out.themes.iter().any(|t| t.id == id) {
                out.themes.push(Found {
                    name: if parsed.name.is_empty() { id.clone() } else { parsed.name },
                    exists: target.join(&id).exists(),
                    source: dir.to_path_buf(),
                    id,
                });
            }
        }
    }
    let mut subdirs: Vec<PathBuf> = fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !p.is_symlink())
        .collect();
    subdirs.sort();
    for sub in subdirs {
        find_themes(&sub, target, out)?;
    }
    Ok(())
}

fn free_id(target: &Path, id: &str) -> String {
    (2..).map(|n| format!("{id}-{n}")).find(|c| !target.join(c).exists()).expect("unbounded")
}

/// Copies the found themes into `target`; returns the ids they landed as.
/// Each theme is built beside the target and swapped in, so a failed copy
/// leaves the old one untouched.
pub fn install(mut prepared: Prepared, conflict: Conflict, target: &Path) -> Result<(Vec<String>, Vec<String>)> {
    fs::create_dir_all(target)?;
    let moving = prepared.unpacked.is_some();
    let mut installed = vec![];
    for theme in &prepared.themes {
        let mut id = theme.id.clone();
        if target.join(&id).exists() {
            match conflict {
                Conflict::Skip => {
                    prepared.notes.push(format!("{id} is already installed; skipped"));
                    continue;
                }
                Conflict::KeepBoth => id = free_id(target, &id),
                Conflict::Replace => {}
            }
        }
        let staging = target.join(format!(".iconic-install-{id}"));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        if let Err(e) = copy::copy_tree(&theme.source, &staging, &prepared.root, moving) {
            fs::remove_dir_all(&staging)?;
            return Err(e);
        }
        let final_path = target.join(&id);
        let old = target.join(format!(".iconic-old-{id}"));
        if fs::symlink_metadata(&final_path).is_ok() {
            if old.exists() {
                fs::remove_dir_all(&old)?;
            }
            // A symlinked theme loses only its link.
            if final_path.is_symlink() {
                fs::remove_file(&final_path)?;
            } else {
                fs::rename(&final_path, &old).with_context(|| format!("Could not replace {id}"))?;
            }
        }
        fs::rename(&staging, &final_path).with_context(|| format!("Could not move {id} into place"))?;
        if old.exists() {
            fs::remove_dir_all(&old)?;
        }
        // The icon cache makes GTK lookups fast.
        let status = Command::new("gtk-update-icon-cache").args(["-f", "-t", "-q"]).arg(&final_path).status()?;
        if !status.success() {
            prepared.notes.push(format!("Could not build the icon cache for {id}"));
        }
        installed.push(id);
    }
    if installed.is_empty() {
        bail!("{}", if prepared.notes.is_empty() { "Nothing was installed".into() } else { prepared.notes.join("; ") });
    }
    Ok((installed, prepared.notes))
}

/// Removes leftovers of an install that was killed midway (over an hour old).
pub fn cleanup_stale(target: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(target) else { return Ok(()) };
    let hour_ago = SystemTime::now() - Duration::from_secs(3600);
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let ours = [".iconic-unpack-", ".iconic-install-", ".iconic-old-"].iter().any(|p| name.starts_with(p));
        let meta = entry.metadata()?;
        if ours && !entry.path().is_symlink() && meta.modified()? < hour_ago {
            fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

pub fn default_target() -> PathBuf {
    paths::user_icons()
}
