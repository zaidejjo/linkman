//! The filesystem engine: everything linkman knows about symlinks lives here.
//!
//! Paths are handled lexically (no canonicalization) so that links to paths which
//! do not exist yet can still be created, inspected and reported on.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use serde::Serialize;

/// Health of a symlink, determined by following it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkStatus {
    /// Target exists (file or directory).
    Ok,
    /// Target does not exist.
    Broken,
    /// The link loops back onto itself.
    Loop,
}

impl LinkStatus {
    pub fn is_ok(self) -> bool {
        self == LinkStatus::Ok
    }
}

/// What a symlink ultimately points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    File,
    Dir,
    Other,
    Missing,
}

impl TargetKind {
    pub fn label(self) -> &'static str {
        match self {
            TargetKind::File => "file",
            TargetKind::Dir => "directory",
            TargetKind::Other => "special",
            TargetKind::Missing => "missing",
        }
    }
}

/// A symlink on disk, with its raw and resolved targets.
#[derive(Debug, Clone, Serialize)]
pub struct LinkInfo {
    pub path: PathBuf,
    pub target: PathBuf,
    /// Absolute, lexically-normalized location the link points at.
    pub resolved: PathBuf,
    pub status: LinkStatus,
    pub kind: TargetKind,
    pub modified: Option<u64>,
}

impl LinkInfo {
    /// Target as it should be shown: relative targets stay readable.
    pub fn display_target(&self) -> String {
        self.target.display().to_string()
    }

    pub fn is_relative(&self) -> bool {
        !self.target.is_absolute()
    }
}

/// Result of walking a symlink chain.
#[derive(Debug, Clone, Serialize)]
pub struct LinkChain {
    pub hops: Vec<PathBuf>,
    pub status: LinkStatus,
    pub final_target: Option<PathBuf>,
}

/// Options for creating a symlink.
#[derive(Debug, Clone, Copy, Default)]
pub struct LinkOptions {
    /// Store the target relative to the link's directory.
    pub relative: bool,
    /// Replace an existing link/file at the link path.
    pub force: bool,
    /// Move an existing entry out of the way to `<name>.bak` instead of deleting it.
    pub backup: bool,
    /// Create missing parent directories for the link.
    pub create_parents: bool,
    /// Describe the work without touching the filesystem.
    pub dry_run: bool,
}

/// What was sitting at the link path before we replaced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExistingKind {
    Symlink,
    File,
    Dir,
    Other,
}

/// Outcome of a `create_link` call.
#[derive(Debug, Clone, Serialize)]
pub struct LinkOutcome {
    pub link: PathBuf,
    pub target: PathBuf,
    /// Exactly what was written into the symlink.
    pub stored_target: PathBuf,
    pub status: LinkStatus,
    pub replaced: Option<ExistingKind>,
    pub backup: Option<PathBuf>,
    pub dry_run: bool,
}

/// Absolute path with `.`/`..` resolved lexically. Never touches the filesystem.
pub fn normalize(path: &Path) -> PathBuf {
    let absolute = path.is_absolute();
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() && !absolute {
                    out.push("..");
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                out.push(component.as_os_str());
            }
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// Turn `path` into an absolute, lexically-normalized path.
pub fn absolutize(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(normalize(path))
    } else {
        Ok(normalize(&std::env::current_dir()?.join(path)))
    }
}

/// Relative path that leads from directory `from_dir` to `to`.
/// Both inputs should be absolute and normalized.
pub fn relative_to(from_dir: &Path, to: &Path) -> PathBuf {
    let from: Vec<Component<'_>> = from_dir.components().collect();
    let dest: Vec<Component<'_>> = to.components().collect();

    let mut common = 0;
    while common < from.len() && common < dest.len() && from[common] == dest[common] {
        common += 1;
    }

    let mut out = PathBuf::new();
    for _ in common..from.len() {
        out.push("..");
    }
    for component in &dest[common..] {
        out.push(component.as_os_str());
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// Tilde-aware, whitespace-trimmed path from user input.
pub fn expand_path(input: &str) -> PathBuf {
    let input = input.trim();
    if (input == "~" || input.starts_with("~/"))
        && let Some(home) = home_dir()
    {
        let rest = input.trim_start_matches('~');
        return home.join(rest.trim_start_matches('/'));
    }
    PathBuf::from(input)
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

fn loop_error(err: &io::Error) -> bool {
    // ELOOP is 40 on Linux, 62 on macOS/BSD.
    matches!(err.raw_os_error(), Some(40) | Some(62))
}

fn kind_of(md: &fs::Metadata) -> TargetKind {
    if md.is_dir() {
        TargetKind::Dir
    } else if md.is_file() {
        TargetKind::File
    } else {
        TargetKind::Other
    }
}

fn modified_secs(md: &fs::Metadata) -> Option<u64> {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// Inspect a single symlink. Returns `None` when `path` is not a symlink.
pub fn inspect(path: &Path) -> io::Result<Option<LinkInfo>> {
    let md = fs::symlink_metadata(path)?;
    if !md.file_type().is_symlink() {
        return Ok(None);
    }
    let target = fs::read_link(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let resolved = normalize(&absolutize(parent)?.join(&target));

    let (status, kind) = match fs::metadata(path) {
        Ok(followed) => (LinkStatus::Ok, kind_of(&followed)),
        Err(err) if loop_error(&err) => (LinkStatus::Loop, TargetKind::Missing),
        Err(_) => (LinkStatus::Broken, TargetKind::Missing),
    };

    Ok(Some(LinkInfo {
        path: path.to_path_buf(),
        target,
        resolved,
        status,
        kind,
        modified: modified_secs(&md),
    }))
}

/// Follow a chain of symlinks by hand (up to 40 hops).
pub fn resolve_chain(path: &Path) -> LinkChain {
    let mut hops = Vec::new();
    let mut current = path.to_path_buf();
    let mut seen = vec![path.to_path_buf()];

    for _ in 0..40 {
        let Some(info) = inspect(&current).ok().flatten() else {
            let status = if current.exists() {
                LinkStatus::Ok
            } else {
                LinkStatus::Broken
            };
            return LinkChain {
                hops,
                status,
                final_target: Some(current),
            };
        };
        if info.status == LinkStatus::Loop {
            return LinkChain {
                hops,
                status: LinkStatus::Loop,
                final_target: None,
            };
        }
        let next = info.resolved.clone();
        hops.push(info.target.clone());
        if seen.contains(&next) {
            return LinkChain {
                hops,
                status: LinkStatus::Loop,
                final_target: None,
            };
        }
        seen.push(next.clone());
        current = next;
    }

    LinkChain {
        hops,
        status: LinkStatus::Loop,
        final_target: None,
    }
}

fn existing_kind(path: &Path) -> Option<ExistingKind> {
    let md = fs::symlink_metadata(path).ok()?;
    let ft = md.file_type();
    Some(if ft.is_symlink() {
        ExistingKind::Symlink
    } else if ft.is_dir() {
        ExistingKind::Dir
    } else if ft.is_file() {
        ExistingKind::File
    } else {
        ExistingKind::Other
    })
}

fn backup_path(path: &Path) -> PathBuf {
    let mut candidate = path.as_os_str().to_os_string();
    candidate.push(".bak");
    let mut candidate = PathBuf::from(candidate);
    let mut n = 1;
    while fs::symlink_metadata(&candidate).is_ok() {
        let mut name = path.as_os_str().to_os_string();
        name.push(format!(".bak.{n}"));
        candidate = PathBuf::from(name);
        n += 1;
    }
    candidate
}

/// Create a symlink at `link_path` pointing at `target`.
pub fn create_link(target: &Path, link_path: &Path, opts: &LinkOptions) -> Result<LinkOutcome> {
    let target_abs = absolutize(target)?;
    let link_abs = absolutize(link_path)?;

    if target_abs == link_abs {
        bail!("refusing to link {} to itself", link_abs.display());
    }

    let parent = link_abs
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/"));

    if opts.create_parents {
        if !opts.dry_run {
            fs::create_dir_all(&parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
    } else if !parent.is_dir() {
        bail!(
            "parent directory does not exist: {} (use --parents to create it)",
            parent.display()
        );
    }

    let stored = if opts.relative {
        relative_to(&parent, &target_abs)
    } else {
        target_abs.clone()
    };

    let mut replaced = None;
    let mut backup = None;

    if let Some(kind) = existing_kind(&link_abs) {
        if opts.backup {
            let dest = backup_path(&link_abs);
            if !opts.dry_run {
                fs::rename(&link_abs, &dest).with_context(|| {
                    format!(
                        "could not move {} to {}",
                        link_abs.display(),
                        dest.display()
                    )
                })?;
            }
            backup = Some(dest);
            replaced = Some(kind);
        } else if opts.force {
            if kind == ExistingKind::Dir {
                bail!(
                    "refusing to replace directory {} (remove it manually)",
                    link_abs.display()
                );
            }
            if !opts.dry_run {
                fs::remove_file(&link_abs)
                    .with_context(|| format!("could not replace {}", link_abs.display()))?;
            }
            replaced = Some(kind);
        } else {
            bail!(
                "already exists: {} (use --force to replace or --backup to keep a copy)",
                link_abs.display()
            );
        }
    }

    let status = match fs::metadata(&target_abs) {
        Ok(_) => LinkStatus::Ok,
        Err(_) => LinkStatus::Broken,
    };

    if !opts.dry_run {
        make_symlink(&stored, &link_abs, &target_abs)?;
    }

    Ok(LinkOutcome {
        link: link_path.to_path_buf(),
        target: target_abs,
        stored_target: stored,
        status,
        replaced,
        backup,
        dry_run: opts.dry_run,
    })
}

#[cfg(unix)]
fn make_symlink(stored: &Path, link_path: &Path, _target_abs: &Path) -> Result<()> {
    std::os::unix::fs::symlink(stored, link_path)
        .with_context(|| format!("could not create symlink {}", link_path.display()))
}

#[cfg(windows)]
fn make_symlink(stored: &Path, link_path: &Path, target_abs: &Path) -> Result<()> {
    let result = if target_abs.is_dir() {
        std::os::windows::fs::symlink_dir(stored, link_path)
    } else {
        std::os::windows::fs::symlink_file(stored, link_path)
    };
    result.with_context(|| format!("could not create symlink {}", link_path.display()))
}

/// Create a symlink from a raw stored target, exactly as it should appear.
/// Used when restoring a link from undo history.
pub fn create_raw_symlink(stored: &Path, link: &Path) -> Result<()> {
    let link_abs = absolutize(link)?;
    let parent = link_abs
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/"));
    let target_abs = normalize(&parent.join(stored));
    make_symlink(stored, &link_abs, &target_abs)
}

/// Remove a symlink. Regular files and directories are never deleted.
pub fn remove_link(path: &Path, dry_run: bool) -> Result<()> {
    let md =
        fs::symlink_metadata(path).with_context(|| format!("no such path: {}", path.display()))?;
    if !md.file_type().is_symlink() {
        bail!("not a symlink: {} (refusing to remove)", path.display());
    }
    if !dry_run {
        fs::remove_file(path).with_context(|| format!("could not remove {}", path.display()))?;
    }
    Ok(())
}

/// All symlinks below `root`, optionally walking real subdirectories.
pub fn scan_links(root: &Path, recursive: bool) -> Result<Vec<LinkInfo>> {
    fs::read_dir(root).with_context(|| format!("cannot read {}", root.display()))?;
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(md) = fs::symlink_metadata(&path) else {
                continue;
            };
            if md.file_type().is_symlink() {
                if let Ok(Some(info)) = inspect(&path) {
                    out.push(info);
                }
            } else if recursive && md.is_dir() && depth < 64 {
                stack.push((path, depth + 1));
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn normalize_handles_dots_and_roots() {
        assert_eq!(normalize(Path::new("/a/./b/../c")), Path::new("/a/c"));
        assert_eq!(normalize(Path::new("a/../../b")), Path::new("../b"));
        assert_eq!(normalize(Path::new("/..")), Path::new("/"));
        assert_eq!(normalize(Path::new("")), Path::new("."));
    }

    #[test]
    fn relative_links_are_short() {
        assert_eq!(
            relative_to(Path::new("/home/me/links"), Path::new("/home/me/src/app")),
            Path::new("../src/app")
        );
        assert_eq!(
            relative_to(Path::new("/home/me"), Path::new("/home/me")),
            Path::new(".")
        );
        assert_eq!(
            relative_to(Path::new("/a/b/c"), Path::new("/a/b/c/d/e")),
            Path::new("d/e")
        );
        assert_eq!(
            relative_to(Path::new("/a/b/c"), Path::new("/x/y")),
            Path::new("../../../x/y")
        );
    }

    #[test]
    fn creates_absolute_and_relative_links() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target.txt");
        fs::write(&target, "hi").unwrap();
        let abs_link = tmp.path().join("abs");
        let out = create_link(&target, &abs_link, &LinkOptions::default()).unwrap();
        assert_eq!(out.status, LinkStatus::Ok);
        assert_eq!(fs::read_link(&abs_link).unwrap(), target);

        let rel_link = tmp.path().join("rel");
        let out = create_link(
            &target,
            &rel_link,
            &LinkOptions {
                relative: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.stored_target, Path::new("target.txt"));
        assert_eq!(fs::read_link(&rel_link).unwrap(), Path::new("target.txt"));
    }

    #[test]
    fn refuses_existing_without_force_and_replaces_with_it() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "x").unwrap();
        let link = tmp.path().join("link");
        fs::write(&link, "old").unwrap();

        let err = create_link(&target, &link, &LinkOptions::default()).unwrap_err();
        assert!(err.to_string().contains("already exists"));

        let out = create_link(
            &target,
            &link,
            &LinkOptions {
                force: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.replaced, Some(ExistingKind::File));
        assert_eq!(fs::read_link(&link).unwrap(), target);
    }

    #[test]
    fn backup_keeps_old_file() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "x").unwrap();
        let link = tmp.path().join("link");
        fs::write(&link, "old").unwrap();

        let out = create_link(
            &target,
            &link,
            &LinkOptions {
                backup: true,
                ..Default::default()
            },
        )
        .unwrap();
        let backup = out.backup.unwrap();
        assert_eq!(fs::read_to_string(backup).unwrap(), "old");
    }

    #[test]
    fn refuses_to_replace_directory() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "x").unwrap();
        let dir = tmp.path().join("dir");
        fs::create_dir(&dir).unwrap();
        let err = create_link(
            &target,
            &dir,
            &LinkOptions {
                force: true,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("refusing to replace directory"));
        assert!(dir.is_dir());
    }

    #[test]
    fn unlink_never_removes_regular_files() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("file");
        fs::write(&file, "x").unwrap();
        assert!(remove_link(&file, false).is_err());
        assert!(file.exists());

        let link = tmp.path().join("link");
        symlink(&file, &link).unwrap();
        remove_link(&link, false).unwrap();
        assert!(!link.exists());
    }

    #[test]
    fn inspects_broken_and_dangling_links() {
        let tmp = tempdir().unwrap();
        let link = tmp.path().join("dangling");
        create_link(
            Path::new("/definitely/not/here"),
            &link,
            &LinkOptions::default(),
        )
        .unwrap();
        let info = inspect(&link).unwrap().unwrap();
        assert_eq!(info.status, LinkStatus::Broken);
        assert_eq!(info.kind, TargetKind::Missing);
        assert_eq!(info.resolved, Path::new("/definitely/not/here"));
    }

    #[test]
    fn chain_detects_loops() {
        let tmp = tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        symlink(&b, &a).unwrap();
        symlink(&a, &b).unwrap();
        let chain = resolve_chain(&a);
        assert_eq!(chain.status, LinkStatus::Loop);
    }

    #[test]
    fn scan_finds_broken_links_recursively() {
        let tmp = tempdir().unwrap();
        let nested = tmp.path().join("nested");
        fs::create_dir(&nested).unwrap();
        let good = tmp.path().join("good");
        fs::write(&good, "x").unwrap();
        symlink(&good, tmp.path().join("fine")).unwrap();
        symlink("/nope", nested.join("bad")).unwrap();
        let links = scan_links(tmp.path(), true).unwrap();
        assert_eq!(links.len(), 2);
        assert_eq!(
            links
                .iter()
                .filter(|l| l.status == LinkStatus::Broken)
                .count(),
            1
        );
        let shallow = scan_links(tmp.path(), false).unwrap();
        assert_eq!(shallow.len(), 1);
    }

    #[test]
    fn dry_run_does_not_touch_disk() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "x").unwrap();
        let link = tmp.path().join("link");
        let out = create_link(
            &target,
            &link,
            &LinkOptions {
                dry_run: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(out.dry_run);
        assert!(fs::symlink_metadata(&link).is_err());
    }
}
