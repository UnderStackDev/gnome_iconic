//! Archives are untrusted: listed first (size and entry limits), then
//! unpacked by bsdtar, which refuses absolute paths and ".." on its own.

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

pub const MAX_UNPACKED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 300_000;

pub fn is_archive(path: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    const ENDINGS: [&str; 13] = [
        ".tar", ".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst", ".tar.lz4", ".tar.lzma", ".tgz", ".txz", ".tbz",
        ".tbz2", ".zip", ".7z",
    ];
    ENDINGS.iter().any(|e| name.ends_with(e))
}

/// Total size and entry count from `bsdtar -tv` output (ls-style lines:
/// perms, links, owner, group, size, ...).
pub fn listing_totals(listing: &str) -> (u64, usize) {
    let mut bytes = 0u64;
    let mut entries = 0usize;
    for line in listing.lines().filter(|l| !l.trim().is_empty()) {
        entries += 1;
        bytes += line.split_whitespace().nth(4).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    }
    (bytes, entries)
}

fn bsdtar(args: &[&std::ffi::OsStr]) -> Result<String> {
    let out = Command::new("bsdtar").args(args).output().context("Could not run bsdtar")?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Checks the limits, then unpacks `archive` into `dest`.
pub fn unpack(archive: &Path, dest: &Path) -> Result<()> {
    let file = archive.file_name().unwrap_or_default().to_string_lossy();
    let listing = bsdtar(&["-tvf".as_ref(), archive.as_os_str()])
        .with_context(|| format!("Could not read the archive {file}"))?;
    let (bytes, entries) = listing_totals(&listing);
    if bytes > MAX_UNPACKED_BYTES || entries > MAX_ENTRIES {
        bail!("{file} is too big to unpack ({} MB in {entries} files)", bytes / (1024 * 1024));
    }
    bsdtar(&[
        "-xf".as_ref(),
        archive.as_os_str(),
        "-C".as_ref(),
        dest.as_os_str(),
        "--no-same-owner".as_ref(),
        "--no-same-permissions".as_ref(),
    ])
    .with_context(|| format!("Could not unpack {file}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_names() {
        assert!(is_archive(Path::new("/x/Papirus.tar.XZ")));
        assert!(is_archive(Path::new("a.zip")));
        assert!(!is_archive(Path::new("a.png")));
    }

    #[test]
    fn listing() {
        let l = "drwxr-xr-x  0 u g       0 Jan  1  2024 Nice/\n-rw-r--r--  0 u g    1234 Jan  1  2024 Nice/index.theme\n";
        assert_eq!(listing_totals(l), (1234, 2));
    }
}
