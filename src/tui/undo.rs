//! Safe, session-local undo records for the TUI.
//!
//! Undo is deliberately conservative: it only removes a link if it still points
//! at exactly the target we wrote, and it never deletes regular files.

use std::fs;
use std::path::{Path, PathBuf};

use crate::fsops;

#[derive(Debug, Clone)]
pub struct CreatedLink {
    pub path: PathBuf,
    pub stored_target: PathBuf,
    pub backup: Option<PathBuf>,
    /// Something existed here and was replaced without a backup.
    pub replaced: bool,
}

#[derive(Debug, Clone)]
pub struct RemovedLink {
    pub path: PathBuf,
    pub raw_target: PathBuf,
}

#[derive(Debug, Clone)]
pub enum UndoOp {
    Link {
        links: Vec<CreatedLink>,
    },
    Remove {
        links: Vec<RemovedLink>,
    },
    Retarget {
        link: PathBuf,
        previous: PathBuf,
        current: PathBuf,
    },
}

impl UndoOp {
    pub fn label(&self) -> &'static str {
        match self {
            UndoOp::Link { .. } => "link",
            UndoOp::Remove { .. } => "remove",
            UndoOp::Retarget { .. } => "retarget",
        }
    }
}

#[derive(Debug, Default)]
pub struct UndoReport {
    pub restored: usize,
    pub skipped: usize,
    pub backups: usize,
    /// Replaced entries that had no backup and cannot be brought back.
    pub lost: usize,
    pub errors: Vec<String>,
}

pub fn apply(op: &UndoOp) -> UndoReport {
    let mut report = UndoReport::default();
    match op {
        UndoOp::Link { links } => undo_links(links, &mut report),
        UndoOp::Remove { links } => undo_remove(links, &mut report),
        UndoOp::Retarget {
            link,
            previous,
            current,
        } => undo_retarget(link, previous, current, &mut report),
    }
    report
}

fn undo_links(links: &[CreatedLink], report: &mut UndoReport) {
    for created in links {
        let mut removed = false;
        if let Ok(md) = fs::symlink_metadata(&created.path) {
            if md.file_type().is_symlink() {
                match fs::read_link(&created.path) {
                    Ok(raw) if raw == created.stored_target => {
                        match fs::remove_file(&created.path) {
                            Ok(()) => {
                                report.restored += 1;
                                removed = true;
                            }
                            Err(err) => report
                                .errors
                                .push(format!("{}: {err}", created.path.display())),
                        }
                    }
                    Ok(_) => report.skipped += 1,
                    Err(err) => report
                        .errors
                        .push(format!("{}: {err}", created.path.display())),
                }
            } else {
                report.skipped += 1;
            }
        }

        if let Some(backup) = &created.backup {
            if fs::symlink_metadata(&created.path).is_err() {
                match fs::rename(backup, &created.path) {
                    Ok(()) => report.backups += 1,
                    Err(err) => report
                        .errors
                        .push(format!("could not restore {}: {err}", backup.display())),
                }
            } else if !removed {
                report.errors.push(format!(
                    "backup kept at {} (path is occupied)",
                    backup.display()
                ));
            }
        } else if created.replaced {
            report.lost += 1;
        }
    }
}

fn undo_remove(links: &[RemovedLink], report: &mut UndoReport) {
    for removed in links {
        if fs::symlink_metadata(&removed.path).is_ok() {
            report.skipped += 1;
            continue;
        }
        match fsops::create_raw_symlink(&removed.raw_target, &removed.path) {
            Ok(()) => report.restored += 1,
            Err(err) => report
                .errors
                .push(format!("{}: {err}", removed.path.display())),
        }
    }
}

fn undo_retarget(link: &Path, previous: &Path, current: &Path, report: &mut UndoReport) {
    let Ok(md) = fs::symlink_metadata(link) else {
        report.skipped += 1;
        return;
    };
    if !md.file_type().is_symlink() {
        report.skipped += 1;
        return;
    }
    match fs::read_link(link) {
        Ok(raw) if raw == current => {}
        Ok(_) => {
            report.skipped += 1;
            return;
        }
        Err(err) => {
            report.errors.push(format!("{}: {err}", link.display()));
            return;
        }
    }

    match replace_with_raw_symlink(link, previous) {
        Ok(()) => report.restored += 1,
        Err(err) => report.errors.push(format!("{}: {err}", link.display())),
    }
}

/// Build the new symlink beside the target and rename it over the old one, so a
/// failure cannot leave the path missing.
fn replace_with_raw_symlink(link: &Path, stored: &Path) -> anyhow::Result<()> {
    let parent = link.parent().unwrap_or_else(|| Path::new("."));
    let name = link
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "link".to_string());
    let mut temp = parent.join(format!(".{name}.linkman-undo"));
    let mut attempt = 0;
    while fs::symlink_metadata(&temp).is_ok() {
        attempt += 1;
        temp = parent.join(format!(".{name}.linkman-undo.{attempt}"));
    }
    fsops::create_raw_symlink(stored, &temp)?;
    match fs::rename(&temp, link) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(&temp);
            Err(err.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn undo_link_removes_it_and_restores_backup() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "new").unwrap();
        let link = tmp.path().join("link");
        fs::write(&link, "old").unwrap();

        let outcome = fsops::create_link(
            &target,
            &link,
            &fsops::LinkOptions {
                backup: true,
                ..Default::default()
            },
        )
        .unwrap();
        let op = UndoOp::Link {
            links: vec![CreatedLink {
                path: link.clone(),
                stored_target: outcome.stored_target,
                backup: outcome.backup,
                replaced: true,
            }],
        };

        let report = apply(&op);
        assert_eq!(report.restored, 1);
        assert_eq!(report.backups, 1);
        assert_eq!(fs::read_to_string(&link).unwrap(), "old");
    }

    #[test]
    fn undo_does_not_touch_a_retargeted_link() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "x").unwrap();
        let other = tmp.path().join("other");
        fs::write(&other, "y").unwrap();
        let link = tmp.path().join("link");
        let outcome = fsops::create_link(&target, &link, &fsops::LinkOptions::default()).unwrap();
        fs::remove_file(&link).unwrap();
        symlink(&other, &link).unwrap();

        let report = apply(&UndoOp::Link {
            links: vec![CreatedLink {
                path: link.clone(),
                stored_target: outcome.stored_target,
                backup: None,
                replaced: false,
            }],
        });
        assert_eq!(report.restored, 0);
        assert_eq!(report.skipped, 1);
        assert_eq!(fs::read_link(&link).unwrap(), other);
    }

    #[test]
    fn undo_remove_recreates_relative_link() {
        let tmp = tempdir().unwrap();
        let link = tmp.path().join("link");
        symlink("target", &link).unwrap();
        fs::remove_file(&link).unwrap();

        let report = apply(&UndoOp::Remove {
            links: vec![RemovedLink {
                path: link.clone(),
                raw_target: PathBuf::from("target"),
            }],
        });
        assert_eq!(report.restored, 1);
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("target"));
    }

    #[test]
    fn undo_retarget_restores_previous_raw_target() {
        let tmp = tempdir().unwrap();
        let link = tmp.path().join("link");
        symlink("../old", &link).unwrap();
        let report = apply(&UndoOp::Retarget {
            link: link.clone(),
            previous: PathBuf::from("../old"),
            current: PathBuf::from("../new"),
        });
        // The link does not point at current, so undo leaves it alone.
        assert_eq!(report.skipped, 1);
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("../old"));

        fs::remove_file(&link).unwrap();
        symlink("../new", &link).unwrap();
        let report = apply(&UndoOp::Retarget {
            link: link.clone(),
            previous: PathBuf::from("../old"),
            current: PathBuf::from("../new"),
        });
        assert_eq!(report.restored, 1);
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("../old"));
    }
}
