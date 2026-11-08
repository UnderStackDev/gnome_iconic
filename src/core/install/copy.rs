//! Copies a theme folder keeping only regular files, folders and symlinks
//! that stay inside the archive and, once installed, inside the icon folder.

use crate::core::removal::clean;
use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Component, Path, PathBuf};

/// `to` relative to the folder `from_dir`, both clean absolute paths.
pub fn relative(from_dir: &Path, to: &Path) -> PathBuf {
    let a: Vec<Component> = from_dir.components().collect();
    let b: Vec<Component> = to.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut out = PathBuf::new();
    for _ in common..a.len() {
        out.push("..");
    }
    for c in &b[common..] {
        out.push(c);
    }
    out
}

/// Copies (or with `moving`, renames) `from` into `to`. Links may point
/// anywhere inside `allowed`, but must land inside `to`'s parent folder.
pub fn copy_tree(from: &Path, to: &Path, allowed: &Path, moving: bool) -> Result<()> {
    let root = fs::canonicalize(from).with_context(|| format!("Could not read {}", from.display()))?;
    let allowed = fs::canonicalize(allowed)?;
    let target_parent = clean(to.parent().context("target has no parent")?);
    fs::create_dir_all(to).with_context(|| format!("Could not create {}", to.display()))?;
    walk(&root, &root, to, &allowed, &target_parent, moving)
}

fn walk(root: &Path, dir: &Path, to: &Path, allowed: &Path, parent: &Path, moving: bool) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let rel = path.strip_prefix(root)?;
        let dest = to.join(rel);
        let meta = fs::symlink_metadata(&path)?;
        let dest_dir = dest.parent().expect("inside to");

        if meta.file_type().is_symlink() {
            // Themes link sizes and aliases to each other, and variants link
            // into a sibling theme (Papirus-Dark -> ../../Papirus). Keep those.
            let raw = fs::read_link(&path)?;
            let resolved = clean(&path.parent().expect("inside root").join(&raw));
            if !resolved.starts_with(allowed) {
                continue;
            }
            let text = if resolved.starts_with(root) {
                relative(&clean(dest_dir), &clean(&to.join(resolved.strip_prefix(root)?)))
            } else if raw.is_absolute() {
                continue;
            } else {
                raw
            };
            if !clean(&dest_dir.join(&text)).starts_with(parent) {
                continue;
            }
            fs::create_dir_all(dest_dir)?;
            let _ = fs::remove_file(&dest);
            symlink(&text, &dest).with_context(|| format!("Could not link {}", rel.display()))?;
        } else if meta.is_dir() {
            fs::create_dir_all(&dest)?;
            walk(root, &path, to, allowed, parent, moving)?;
        } else if meta.is_file() {
            fs::create_dir_all(dest_dir)?;
            if !(moving && fs::rename(&path, &dest).is_ok()) {
                fs::copy(&path, &dest).with_context(|| format!("Could not copy {}", rel.display()))?;
            }
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o644))?;
        }
        // Devices, sockets and fifos are dropped.
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_inner_links_and_drops_escaping_ones() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src/Nice");
        fs::create_dir_all(src.join("48/apps")).unwrap();
        fs::write(src.join("48/apps/a.svg"), "<svg/>").unwrap();
        symlink("a.svg", src.join("48/apps/b.svg")).unwrap();
        symlink("/etc/passwd", src.join("48/apps/evil.svg")).unwrap();
        symlink("../../../../../etc", src.join("48/up")).unwrap();

        let dest = dir.path().join("icons/Nice");
        copy_tree(&src, &dest, &dir.path().join("src"), false).unwrap();
        assert!(dest.join("48/apps/a.svg").is_file());
        assert_eq!(fs::read_link(dest.join("48/apps/b.svg")).unwrap(), PathBuf::from("a.svg"));
        assert!(!dest.join("48/apps/evil.svg").exists());
        assert!(fs::symlink_metadata(dest.join("48/up")).is_err());
    }

    #[test]
    fn relative_paths() {
        assert_eq!(relative(Path::new("/a/b/c"), Path::new("/a/d")), PathBuf::from("../../d"));
    }
}
