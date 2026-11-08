//! Installed icon themes, read from their index.theme files.

use super::{ini, paths};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ThemeInfo {
    /// Folder name: what gsettings stores.
    pub id: String,
    /// Name= for display.
    pub name: String,
    pub comment: String,
    /// The folder that wins (user folders shadow system ones).
    pub path: PathBuf,
    /// Lives in the user's own icon folders, so it can be removed.
    pub user: bool,
}

#[derive(Clone, Debug)]
pub struct Directory {
    pub path: String,
    pub context: String,
}

#[derive(Clone, Debug, Default)]
pub struct Index {
    pub name: String,
    pub comment: String,
    pub inherits: Vec<String>,
    pub directories: Vec<Directory>,
    pub hidden: bool,
}

/// Reads one index.theme; None if it isn't an icon theme index.
pub fn parse_index(file: &Path) -> Option<Index> {
    let sections = ini::read(file);
    let head = sections.get("Icon Theme")?;
    let get = |k: &str| head.get(k).cloned().unwrap_or_default();
    let mut dirs = ini::list(head.get("Directories"));
    dirs.extend(ini::list(head.get("ScaledDirectories")));
    let mut seen = BTreeSet::new();
    let directories = dirs
        .into_iter()
        .filter(|d| seen.insert(d.clone()))
        .filter_map(|d| {
            let keys = sections.get(&d)?;
            let size: u32 = keys.get("Size")?.parse().ok()?;
            (size > 0).then(|| Directory { context: keys.get("Context").cloned().unwrap_or_default(), path: d })
        })
        .collect();
    Some(Index {
        name: get("Name"),
        comment: get("Comment"),
        inherits: ini::list(head.get("Inherits")),
        directories,
        hidden: get("Hidden").eq_ignore_ascii_case("true"),
    })
}

/// Themes worth showing, sorted by folder name: they have icon folders (so
/// cursor-only themes drop out) and aren't Hidden (Adwaita aside).
pub fn scan() -> Vec<ThemeInfo> {
    let user_roots = [paths::user_icons(), paths::home().join(".icons")];
    scan_in(&paths::icon_search_paths(), &user_roots)
}

pub fn scan_in(search: &[PathBuf], user_roots: &[PathBuf]) -> Vec<ThemeInfo> {
    const SKIPPED: [&str; 3] = ["hicolor", "default", "locolor"];
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for base in search {
        let Ok(entries) = fs::read_dir(base) else { continue };
        for entry in entries.flatten() {
            let id = entry.file_name().to_string_lossy().into_owned();
            if SKIPPED.contains(&id.as_str()) || id.starts_with('.') || seen.contains(&id) {
                continue;
            }
            let path = entry.path();
            let Some(index) = parse_index(&path.join("index.theme")) else { continue };
            // The first folder in search order wins, like icon lookup does.
            seen.insert(id.clone());
            if (index.hidden && id != "Adwaita") || index.directories.is_empty() {
                continue;
            }
            out.push(ThemeInfo {
                name: if index.name.is_empty() { id.clone() } else { index.name },
                comment: index.comment,
                user: user_roots.contains(base),
                path,
                id,
            });
        }
    }
    out.sort_by_key(|t| t.id.to_lowercase());
    out
}

/// Every app icon name in the theme, the themes it inherits and hicolor:
/// what a launcher can sensibly use. Sorted, no -symbolic ones.
pub fn app_icon_names(theme: &str) -> Vec<String> {
    let search = paths::icon_search_paths();
    let mut names = BTreeSet::new();
    let mut queue = vec![theme.to_string(), "hicolor".to_string()];
    let mut visited = BTreeSet::new();
    while let Some(id) = queue.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        // A theme may be split over several base folders; each is a root.
        let roots: Vec<PathBuf> = search.iter().map(|b| b.join(&id)).filter(|r| r.is_dir()).collect();
        let Some(index) = roots.iter().find_map(|r| parse_index(&r.join("index.theme"))) else { continue };
        for dir in &index.directories {
            let apps = dir.context == "Applications" || (dir.context.is_empty() && dir.path.contains("apps"));
            if !apps {
                continue;
            }
            for root in &roots {
                let Ok(files) = fs::read_dir(root.join(&dir.path)) else { continue };
                for file in files.flatten() {
                    let p = file.path();
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
                    if matches!(ext, "svg" | "png" | "xpm") && !stem.ends_with("-symbolic") {
                        names.insert(stem.to_string());
                    }
                }
            }
        }
        queue.extend(index.inherits);
    }
    let mut out: Vec<String> = names.into_iter().collect();
    out.sort_by_key(|n| n.to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_index_and_skips_cursor_themes() {
        let dir = tempfile::tempdir().unwrap();
        let icons = dir.path().join("Nice");
        fs::create_dir_all(&icons).unwrap();
        fs::write(
            icons.join("index.theme"),
            "[Icon Theme]\nName=Nice Icons\nInherits=Adwaita\nDirectories=48x48/apps\n\n[48x48/apps]\nSize=48\nContext=Applications\n",
        )
        .unwrap();
        let cursor = dir.path().join("Pointer");
        fs::create_dir_all(&cursor).unwrap();
        fs::write(cursor.join("index.theme"), "[Icon Theme]\nName=Pointer\n").unwrap();

        let found = scan_in(&[dir.path().to_path_buf()], &[dir.path().to_path_buf()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "Nice");
        assert_eq!(found[0].name, "Nice Icons");
        assert!(found[0].user);
    }
}
