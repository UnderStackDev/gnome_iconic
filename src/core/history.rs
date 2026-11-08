//! Undo and redo for launcher changes. A snapshot keeps the bytes of every
//! file a change can touch (or that it was absent), so both are exact.

use super::{ini, paths};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Snapshot {
    id: String,
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
}

/// The files Iconic may write for launcher `id`.
pub fn snapshot(id: &str) -> Snapshot {
    let files = [
        paths::user_applications().join(format!("{id}.desktop")),
        paths::backups().join(format!("{id}.desktop")),
    ];
    Snapshot { id: id.to_string(), files: files.into_iter().map(|p| { let d = fs::read(&p).ok(); (p, d) }).collect() }
}

fn restore(snap: &Snapshot) -> Result<()> {
    for (path, data) in &snap.files {
        match data {
            Some(bytes) => ini::write_atomic(path, bytes)?,
            None if path.exists() => fs::remove_file(path)?,
            None => {}
        }
    }
    Ok(())
}

#[derive(Default)]
pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl History {
    /// Records the state before a change; a new change clears redo.
    pub fn push(&mut self, before: Snapshot) {
        self.undo.push(before);
        self.redo.clear();
    }

    /// Returns the launcher id that changed back, if any.
    pub fn undo(&mut self) -> Result<Option<String>> {
        let Some(snap) = self.undo.pop() else { return Ok(None) };
        self.redo.push(snapshot(&snap.id));
        restore(&snap)?;
        Ok(Some(snap.id))
    }

    pub fn redo(&mut self) -> Result<Option<String>> {
        let Some(snap) = self.redo.pop() else { return Ok(None) };
        self.undo.push(snapshot(&snap.id));
        restore(&snap)?;
        Ok(Some(snap.id))
    }
}
