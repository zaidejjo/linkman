//! Directory entries as the TUI sees them: kind, size, permissions, link target.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::fsops::{self, LinkStatus, TargetKind};
use crate::util;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Parent,
    Dir,
    File,
    Link,
    Broken,
    Loop,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified: Option<u64>,
    pub target: Option<PathBuf>,
    pub resolved: Option<PathBuf>,
    pub mode: Option<u32>,
    pub hidden: bool,
}

impl Entry {
    pub fn is_parent(&self) -> bool {
        self.kind == EntryKind::Parent
    }

    pub fn is_link(&self) -> bool {
        matches!(
            self.kind,
            EntryKind::Link | EntryKind::Broken | EntryKind::Loop
        )
    }

    pub fn right_label(&self) -> String {
        match self.kind {
            EntryKind::Parent => String::new(),
            EntryKind::Dir => "dir".to_string(),
            EntryKind::File => util::human_size(self.size.unwrap_or(0)),
            EntryKind::Link => "link".to_string(),
            EntryKind::Broken => "broken".to_string(),
            EntryKind::Loop => "loop".to_string(),
        }
    }

    pub fn mode_string(&self) -> Option<String> {
        let mode = self.mode?;
        let mut out = String::new();
        for shift in [6, 3, 0] {
            let bits = (mode >> shift) & 0o7;
            out.push(if bits & 4 != 0 { 'r' } else { '-' });
            out.push(if bits & 2 != 0 { 'w' } else { '-' });
            out.push(if bits & 1 != 0 { 'x' } else { '-' });
        }
        Some(out)
    }
}

pub fn parent_entry(dir: &Path) -> Option<Entry> {
    dir.parent().map(|parent| Entry {
        name: "..".to_string(),
        path: parent.to_path_buf(),
        kind: EntryKind::Parent,
        is_dir: true,
        size: None,
        modified: None,
        target: None,
        resolved: None,
        mode: None,
        hidden: false,
    })
}

pub fn load_dir(dir: &Path, show_hidden: bool) -> io::Result<Vec<Entry>> {
    let mut out = Vec::new();
    if let Some(parent) = parent_entry(dir) {
        out.push(parent);
    }

    let mut entries = Vec::new();
    for item in fs::read_dir(dir)? {
        let Ok(item) = item else {
            continue;
        };
        let name = item.file_name().to_string_lossy().into_owned();
        let hidden = name.starts_with('.');
        if hidden && !show_hidden {
            continue;
        }
        let path = item.path();
        let Ok(md) = fs::symlink_metadata(&path) else {
            continue;
        };

        let entry = if md.file_type().is_symlink() {
            link_entry(name, path, hidden)
        } else {
            let is_dir = md.is_dir();
            Entry {
                name,
                path,
                kind: if is_dir {
                    EntryKind::Dir
                } else {
                    EntryKind::File
                },
                is_dir,
                size: if md.is_file() { Some(md.len()) } else { None },
                modified: modified_secs(&md),
                target: None,
                resolved: None,
                mode: permissions(&md),
                hidden,
            }
        };
        entries.push(entry);
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out.extend(entries);
    Ok(out)
}

fn link_entry(name: String, path: PathBuf, hidden: bool) -> Entry {
    let info = fsops::inspect(&path).ok().flatten();
    let target_md = fs::metadata(&path).ok();
    let (kind, target, resolved, status_kind, modified) = match &info {
        Some(info) => (
            match info.status {
                LinkStatus::Ok => EntryKind::Link,
                LinkStatus::Broken => EntryKind::Broken,
                LinkStatus::Loop => EntryKind::Loop,
            },
            Some(info.target.clone()),
            Some(info.resolved.clone()),
            Some(info.kind),
            info.modified,
        ),
        None => (
            EntryKind::Broken,
            None,
            None,
            Some(TargetKind::Missing),
            None,
        ),
    };
    Entry {
        name,
        path,
        kind,
        is_dir: status_kind == Some(TargetKind::Dir),
        size: target_md.as_ref().map(|md| md.len()),
        modified,
        target,
        resolved,
        mode: target_md.as_ref().and_then(permissions),
        hidden,
    }
}

fn modified_secs(md: &fs::Metadata) -> Option<u64> {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

#[cfg(unix)]
fn permissions(md: &fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(md.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn permissions(_md: &fs::Metadata) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn lists_dirs_first_with_parent() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("b.txt"), "hello").unwrap();
        fs::create_dir(tmp.path().join("adir")).unwrap();
        fs::write(tmp.path().join(".hidden"), "x").unwrap();

        let entries = load_dir(tmp.path(), false).unwrap();
        assert_eq!(entries[0].kind, EntryKind::Parent);
        assert_eq!(entries[1].name, "adir");
        assert_eq!(entries[2].name, "b.txt");
        assert!(entries.iter().all(|e| e.name != ".hidden"));
        assert_eq!(entries[2].size, Some(5));

        let entries = load_dir(tmp.path(), true).unwrap();
        assert!(entries.iter().any(|e| e.name == ".hidden"));
    }

    #[test]
    fn detects_links_and_broken_links() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "x").unwrap();
        symlink(&target, tmp.path().join("good")).unwrap();
        symlink("/nothing/here", tmp.path().join("bad")).unwrap();

        let entries = load_dir(tmp.path(), false).unwrap();
        let good = entries.iter().find(|e| e.name == "good").unwrap();
        assert_eq!(good.kind, EntryKind::Link);
        assert!(good.target.is_some());
        let bad = entries.iter().find(|e| e.name == "bad").unwrap();
        assert_eq!(bad.kind, EntryKind::Broken);
        assert_eq!(bad.right_label(), "broken");
    }
}
