//! TUI state machine: navigation, marking, linking, prompts and mouse handling.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use super::entry::{self, Entry, EntryKind};
use super::input::TextInput;
use super::palette::{PaletteAction, PaletteCommand, PaletteItem, PaletteState};
use super::theme;
use super::ui::{ModalAction, ToolbarAction, UiMap};
use super::undo::{self, CreatedLink, RemovedLink, UndoOp};
use crate::config::Config;
use crate::fsops::{self, LinkOptions};
use crate::util;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Error,
}

pub struct Toast {
    pub text: String,
    pub level: ToastLevel,
    pub expires: Instant,
}

pub struct PromptState {
    pub title: String,
    pub message: String,
    pub hint: String,
    pub input: TextInput,
    pub kind: PromptKind,
}

pub enum PromptKind {
    Filter(usize),
    GoTo(usize),
    LinkName(PathBuf),
}

pub struct ConfirmState {
    pub title: String,
    pub lines: Vec<String>,
    pub action: ConfirmAction,
}

pub enum ConfirmAction {
    RemoveLinks(Vec<PathBuf>),
}

pub struct ConflictState {
    pub sources: Vec<PathBuf>,
    pub dest: PathBuf,
    pub names: Vec<String>,
    pub conflicts: Vec<String>,
    pub choice: usize,
}

pub enum Mode {
    Normal,
    Help,
    Prompt(PromptState),
    Confirm(ConfirmState),
    Conflict(ConflictState),
    Palette(PaletteState),
}

pub struct Pane {
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: usize,
    pub filter: String,
    pub error: Option<String>,
}

impl Pane {
    fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            entries: Vec::new(),
            selected: 0,
            filter: String::new(),
            error: None,
        }
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        if self.filter.trim().is_empty() {
            return (0..self.entries.len()).collect();
        }
        let needle = self.filter.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.is_parent() || entry.name.to_lowercase().contains(&needle))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn visible_count(&self) -> usize {
        self.visible_indices().len()
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        let visible = self.visible_indices();
        visible
            .get(self.selected)
            .and_then(|index| self.entries.get(*index))
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.selected_entry().map(|entry| entry.path.clone())
    }
}

pub struct App {
    pub panes: [Pane; 2],
    pub active: usize,
    pub mode: Mode,
    pub relative: bool,
    pub force: bool,
    pub show_hidden: bool,
    pub marks: HashSet<PathBuf>,
    pub toast: Option<Toast>,
    pub quit: bool,
    pub map: UiMap,
    undo: Vec<UndoOp>,
    redo: Vec<UndoOp>,
    undo_limit: usize,
    last_click: Option<(Instant, u16, u16)>,
}

impl App {
    pub fn new(source: PathBuf, dest: PathBuf, config: &Config) -> Self {
        let mut app = Self {
            panes: [Pane::new(source), Pane::new(dest)],
            active: 0,
            mode: Mode::Normal,
            relative: config.relative,
            force: config.force,
            show_hidden: config.show_hidden,
            marks: HashSet::new(),
            toast: None,
            quit: false,
            map: UiMap::empty(),
            undo: Vec::new(),
            redo: Vec::new(),
            undo_limit: config.undo_limit.clamp(1, 500),
            last_click: None,
        };
        app.load_pane(0, false);
        app.load_pane(1, false);
        app.set_toast(
            ToastLevel::Info,
            "welcome — Ctrl+P finds anything, ? shows every key",
        );
        app
    }

    fn push_undo(&mut self, op: UndoOp) {
        self.undo.push(op);
        while self.undo.len() > self.undo_limit {
            self.undo.remove(0);
        }
        // A brand new action makes the old redo history misleading.
        self.redo.clear();
    }

    // ---------------------------------------------------------------- toasts

    pub fn set_toast(&mut self, level: ToastLevel, text: impl Into<String>) {
        self.toast = Some(Toast {
            text: text.into(),
            level,
            expires: Instant::now() + Duration::from_secs(6),
        });
    }

    pub fn tick(&mut self) {
        if let Some(toast) = &self.toast
            && Instant::now() >= toast.expires
        {
            self.toast = None;
        }
    }

    // ------------------------------------------------------------ navigation

    fn load_pane(&mut self, index: usize, keep_selection: bool) {
        let previous = if keep_selection {
            self.panes[index].selected_path()
        } else {
            None
        };
        let cwd = self.panes[index].cwd.clone();
        match entry::load_dir(&cwd, self.show_hidden) {
            Ok(entries) => {
                self.panes[index].entries = entries;
                self.panes[index].error = None;
            }
            Err(err) => {
                self.panes[index].entries = entry::parent_entry(&cwd).into_iter().collect();
                self.panes[index].error = Some(err.to_string());
            }
        }
        if let Some(path) = previous {
            self.select_path_in(index, &path);
        } else {
            let pane = &mut self.panes[index];
            pane.selected = if pane.visible_count() > 1 { 1 } else { 0 };
        }
    }

    fn reload_all(&mut self) {
        self.load_pane(0, true);
        self.load_pane(1, true);
    }

    fn select_path_in(&mut self, index: usize, path: &Path) {
        let pane = &self.panes[index];
        let found = pane
            .visible_indices()
            .into_iter()
            .position(|entry_index| pane.entries[entry_index].path == path);
        let pane = &mut self.panes[index];
        match found {
            Some(position) => pane.selected = position,
            None => {
                let count = pane.visible_count();
                pane.selected = pane.selected.min(count.saturating_sub(1));
            }
        }
    }

    fn selected(&self, index: usize) -> Option<Entry> {
        self.panes[index].selected_entry().cloned()
    }

    pub fn move_selection(&mut self, delta: isize) {
        self.move_pane_selection(self.active, delta);
    }

    pub fn move_pane_selection(&mut self, index: usize, delta: isize) {
        let pane = &mut self.panes[index];
        let count = pane.visible_count();
        if count == 0 {
            return;
        }
        let next = (pane.selected as isize + delta).clamp(0, count as isize - 1);
        pane.selected = next as usize;
    }

    fn open_selected(&mut self) {
        let Some(entry) = self.selected(self.active) else {
            return;
        };
        if entry.is_parent() || (entry.is_dir && entry.kind != EntryKind::Broken) {
            self.panes[self.active].cwd = entry.path;
            self.panes[self.active].filter.clear();
            self.load_pane(self.active, false);
        } else if entry.is_link() {
            self.set_toast(
                ToastLevel::Info,
                format!("{} is a link to a file, not a folder", entry.name),
            );
        } else {
            self.set_toast(ToastLevel::Info, format!("{} is not a folder", entry.name));
        }
    }

    fn go_parent(&mut self) {
        let cwd = self.panes[self.active].cwd.clone();
        let Some(parent) = cwd.parent() else {
            self.set_toast(ToastLevel::Info, "already at the filesystem root");
            return;
        };
        self.panes[self.active].cwd = parent.to_path_buf();
        self.panes[self.active].filter.clear();
        self.load_pane(self.active, false);
        self.select_path_in(self.active, &cwd);
    }

    fn go_home(&mut self) {
        let Some(home) = fsops::home_dir() else {
            self.set_toast(ToastLevel::Error, "no home directory in $HOME");
            return;
        };
        self.panes[self.active].cwd = home;
        self.panes[self.active].filter.clear();
        self.load_pane(self.active, false);
        self.set_toast(ToastLevel::Info, "opened home");
    }

    fn goto(&mut self, index: usize, raw: &str) {
        if raw.trim().is_empty() {
            return;
        }
        let path = fsops::expand_path(raw);
        let absolute = match fsops::absolutize(&path) {
            Ok(path) => path,
            Err(err) => {
                self.set_toast(ToastLevel::Error, err.to_string());
                return;
            }
        };
        if absolute.is_dir() {
            self.panes[index].cwd = absolute;
            self.panes[index].filter.clear();
            self.load_pane(index, false);
            self.active = index;
        } else if absolute.exists() {
            if let Some(parent) = absolute.parent() {
                self.panes[index].cwd = parent.to_path_buf();
                self.panes[index].filter.clear();
                self.load_pane(index, false);
                self.select_path_in(index, &absolute);
                self.active = index;
            }
        } else {
            self.set_toast(
                ToastLevel::Error,
                format!("no such path: {}", util::shorten_path(&absolute, 60)),
            );
        }
    }

    // ---------------------------------------------------------------- marks

    fn toggle_mark(&mut self) {
        let Some(entry) = self.selected(self.active) else {
            return;
        };
        if entry.is_parent() {
            return;
        }
        if !self.marks.remove(&entry.path) {
            self.marks.insert(entry.path);
        }
    }

    fn mark_all(&mut self) {
        let pane = &self.panes[self.active];
        let visible: Vec<PathBuf> = pane
            .visible_indices()
            .into_iter()
            .map(|index| &pane.entries[index])
            .filter(|entry| !entry.is_parent())
            .map(|entry| entry.path.clone())
            .collect();
        let all_marked =
            !visible.is_empty() && visible.iter().all(|path| self.marks.contains(path));
        for path in visible {
            if all_marked {
                self.marks.remove(&path);
            } else {
                self.marks.insert(path);
            }
        }
    }

    fn clear_marks(&mut self) {
        if self.marks.is_empty() {
            return;
        }
        self.marks.clear();
        self.set_toast(ToastLevel::Info, "marks cleared");
    }

    fn marked_in(&self, index: usize) -> Vec<PathBuf> {
        let pane = &self.panes[index];
        pane.visible_indices()
            .into_iter()
            .map(|entry_index| &pane.entries[entry_index])
            .filter(|entry| !entry.is_parent() && self.marks.contains(&entry.path))
            .map(|entry| entry.path.clone())
            .collect()
    }

    fn source_targets(&self) -> Vec<PathBuf> {
        let marked = self.marked_in(0);
        if !marked.is_empty() {
            return marked;
        }
        self.selected(0)
            .filter(|entry| !entry.is_parent())
            .map(|entry| vec![entry.path])
            .unwrap_or_default()
    }

    // --------------------------------------------------------------- linking

    fn link_action(&mut self) {
        let sources = self.source_targets();
        if sources.is_empty() {
            self.set_toast(
                ToastLevel::Error,
                "select or mark something in SOURCE first",
            );
            return;
        }
        let names: Vec<String> = sources
            .iter()
            .map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "link".to_string())
            })
            .collect();
        let dest = self.panes[1].cwd.clone();
        self.start_link(sources, dest, names);
    }

    fn start_link(&mut self, sources: Vec<PathBuf>, dest: PathBuf, names: Vec<String>) {
        let conflicts: Vec<String> = names
            .iter()
            .filter(|name| fs::symlink_metadata(dest.join(name)).is_ok())
            .cloned()
            .collect();
        if conflicts.is_empty() || self.force {
            self.run_link(sources, dest, names, self.force, false, false);
        } else {
            self.mode = Mode::Conflict(ConflictState {
                sources,
                dest,
                names,
                conflicts,
                choice: 0,
            });
        }
    }

    fn run_link(
        &mut self,
        sources: Vec<PathBuf>,
        dest: PathBuf,
        names: Vec<String>,
        overwrite: bool,
        backup: bool,
        skip_existing: bool,
    ) {
        let opts = LinkOptions {
            relative: self.relative,
            force: overwrite,
            backup,
            create_parents: false,
            dry_run: false,
        };
        let mut created = Vec::new();
        let mut created_sources = Vec::new();
        let mut undo_links = Vec::new();
        let mut backups = 0usize;
        let mut skipped = 0usize;
        let mut errors = Vec::new();

        for (source, name) in sources.iter().zip(names.iter()) {
            let link = dest.join(name);
            if skip_existing && fs::symlink_metadata(&link).is_ok() {
                skipped += 1;
                continue;
            }
            match fsops::create_link(source, &link, &opts) {
                Ok(outcome) => {
                    if outcome.backup.is_some() {
                        backups += 1;
                    }
                    undo_links.push(CreatedLink {
                        path: link.clone(),
                        stored_target: outcome.stored_target.clone(),
                        backup: outcome.backup.clone(),
                        replaced: outcome.replaced.is_some(),
                    });
                    created.push(link);
                    created_sources.push(source.clone());
                }
                Err(err) => errors.push(format!("{name}: {err}")),
            }
        }

        if !undo_links.is_empty() {
            self.push_undo(UndoOp::Link { links: undo_links });
        }
        for source in &created_sources {
            self.marks.remove(source);
        }
        self.load_pane(1, true);
        if let Some(last) = created.last() {
            self.select_path_in(1, last);
        }

        let dest_label = util::shorten_path(&dest, 48);
        let mut message = if created.is_empty() {
            format!("no new links in {dest_label}")
        } else if created.len() == 1 {
            format!("linked 1 item → {dest_label}")
        } else {
            format!("linked {} items → {dest_label}", created.len())
        };
        if skipped > 0 {
            message.push_str(&format!(
                " · skipped {skipped} existing {}",
                if skipped == 1 { "entry" } else { "entries" }
            ));
        }
        if backups > 0 {
            message.push_str(&format!(
                " · {backups} backup{} kept",
                if backups == 1 { "" } else { "s" }
            ));
        }
        if errors.is_empty() {
            if created.is_empty() {
                self.set_toast(ToastLevel::Info, message);
            } else {
                self.set_toast(ToastLevel::Success, message);
            }
        } else {
            let detail = errors.join(" · ");
            message.push_str(&format!(" · {}", util::shorten_middle(&detail, 90)));
            self.set_toast(ToastLevel::Error, message);
        }
    }

    fn prompt_custom_name(&mut self) {
        let sources = self.source_targets();
        match sources.as_slice() {
            [] => self.set_toast(
                ToastLevel::Error,
                "select or mark something in SOURCE first",
            ),
            [source] => self.open_prompt(PromptKind::LinkName(source.clone())),
            _ => self.set_toast(
                ToastLevel::Error,
                "custom names work one link at a time — clear marks first",
            ),
        }
    }

    fn retarget(&mut self) {
        let Some(link) = self.selected(1) else {
            self.set_toast(ToastLevel::Error, "select a symlink in DEST first");
            return;
        };
        if !link.is_link()
            || !fs::symlink_metadata(&link.path)
                .map(|md| md.file_type().is_symlink())
                .unwrap_or(false)
        {
            self.set_toast(
                ToastLevel::Error,
                format!("{} is not a symlink anymore", link.name),
            );
            return;
        }
        let marked = self.marked_in(0);
        let target = match marked.as_slice() {
            [single] => single.clone(),
            [] => match self.selected(0).filter(|entry| !entry.is_parent()) {
                Some(entry) => entry.path,
                None => {
                    self.set_toast(ToastLevel::Error, "select a new target in SOURCE");
                    return;
                }
            },
            _ => {
                self.set_toast(ToastLevel::Error, "mark exactly one source to retarget");
                return;
            }
        };
        let previous = match fs::read_link(&link.path) {
            Ok(target) => target,
            Err(err) => {
                self.set_toast(ToastLevel::Error, err.to_string());
                return;
            }
        };
        let opts = LinkOptions {
            relative: self.relative,
            force: true,
            backup: false,
            create_parents: false,
            dry_run: false,
        };
        match fsops::create_link(&target, &link.path, &opts) {
            Ok(outcome) => {
                self.push_undo(UndoOp::Retarget {
                    link: link.path.clone(),
                    previous,
                    current: outcome.stored_target.clone(),
                });
                self.load_pane(1, true);
                self.select_path_in(1, &link.path);
                self.set_toast(
                    ToastLevel::Success,
                    format!(
                        "retargeted {} → {}",
                        link.name,
                        util::shorten_path(&target, 48)
                    ),
                );
            }
            Err(err) => self.set_toast(ToastLevel::Error, err.to_string()),
        }
    }

    // --------------------------------------------------------------- removal

    fn confirm_remove(&mut self) {
        let marked = self.marked_in(self.active);
        let paths = if marked.is_empty() {
            match self.selected(self.active) {
                Some(entry) if entry.is_link() => vec![entry.path],
                _ => Vec::new(),
            }
        } else {
            marked
        };
        let paths: Vec<PathBuf> = paths
            .into_iter()
            .filter(|path| {
                fs::symlink_metadata(path)
                    .map(|md| md.file_type().is_symlink())
                    .unwrap_or(false)
            })
            .collect();
        if paths.is_empty() {
            self.set_toast(
                ToastLevel::Error,
                "select a symlink first — regular files are never deleted",
            );
            return;
        }
        let mut lines = Vec::new();
        lines.push(format!(
            "{} symlink{} will be removed:",
            paths.len(),
            if paths.len() == 1 { "" } else { "s" }
        ));
        for path in paths.iter().take(5) {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            lines.push(format!("  • {name}"));
        }
        if paths.len() > 5 {
            lines.push(format!("  … and {} more", paths.len() - 5));
        }
        self.mode = Mode::Confirm(ConfirmState {
            title: "Remove symlinks".to_string(),
            lines,
            action: ConfirmAction::RemoveLinks(paths),
        });
    }

    fn do_remove(&mut self, paths: Vec<PathBuf>) {
        let mut removed = 0usize;
        let mut undo_links = Vec::new();
        let mut errors = Vec::new();
        for path in &paths {
            let raw_target = fsops::inspect(path).ok().flatten().map(|info| info.target);
            match fsops::remove_link(path, false) {
                Ok(()) => {
                    removed += 1;
                    if let Some(raw_target) = raw_target {
                        undo_links.push(RemovedLink {
                            path: path.clone(),
                            raw_target,
                        });
                    }
                    self.marks.remove(path);
                }
                Err(err) => errors.push(err.to_string()),
            }
        }
        if !undo_links.is_empty() {
            self.push_undo(UndoOp::Remove { links: undo_links });
        }
        self.load_pane(0, true);
        self.load_pane(1, true);
        if errors.is_empty() {
            self.set_toast(
                ToastLevel::Success,
                format!(
                    "removed {removed} symlink{}",
                    if removed == 1 { "" } else { "s" }
                ),
            );
        } else {
            self.set_toast(
                ToastLevel::Error,
                format!(
                    "removed {removed} · {} failed: {}",
                    errors.len(),
                    util::shorten_middle(&errors.join(" · "), 80)
                ),
            );
        }
    }

    fn undo_last(&mut self) {
        let Some(op) = self.undo.pop() else {
            self.set_toast(ToastLevel::Info, "nothing to undo yet");
            return;
        };
        self.redo.push(op.clone());
        self.trim_redo();
        let report = undo::apply(&op);
        self.after_history_change("undo", op.label(), report);
    }

    fn redo_last(&mut self) {
        let Some(op) = self.redo.pop() else {
            self.set_toast(
                ToastLevel::Info,
                "nothing to redo — undo something first (Ctrl+Z)",
            );
            return;
        };
        let report = undo::reapply(&op);
        self.undo.push(op.clone());
        while self.undo.len() > self.undo_limit {
            self.undo.remove(0);
        }
        self.after_history_change("redo", op.label(), report);
    }

    fn trim_redo(&mut self) {
        while self.redo.len() > self.undo_limit {
            self.redo.remove(0);
        }
    }

    fn after_history_change(&mut self, verb: &str, label: &str, report: undo::UndoReport) {
        self.load_pane(0, true);
        self.load_pane(1, true);
        let mut parts = Vec::new();
        if report.restored > 0 {
            parts.push(if verb == "redo" {
                format!(
                    "applied {} change{}",
                    report.restored,
                    if report.restored == 1 { "" } else { "s" }
                )
            } else {
                format!(
                    "restored {} link{}",
                    report.restored,
                    if report.restored == 1 { "" } else { "s" }
                )
            });
        }
        if report.backups > 0 {
            parts.push(format!(
                "{} backup{} {}",
                report.backups,
                if report.backups == 1 { "" } else { "s" },
                if verb == "redo" { "recreated" } else { "back" }
            ));
        }
        if report.skipped > 0 {
            parts.push(format!("{} skipped", report.skipped));
        }
        if report.lost > 0 {
            parts.push(format!("{} replaced entry not recoverable", report.lost));
        }
        if report.errors.is_empty() {
            if parts.is_empty() {
                self.set_toast(
                    ToastLevel::Info,
                    format!("{verb} {label}: nothing to change"),
                );
            } else {
                self.set_toast(
                    ToastLevel::Success,
                    format!("{verb} {label} · {}", parts.join(" · ")),
                );
            }
        } else {
            let detail = report.errors.join(" · ");
            let mut message = format!("{verb} {label}");
            if !parts.is_empty() {
                message.push_str(&format!(" · {}", parts.join(" · ")));
            }
            message.push_str(&format!(" · {}", util::shorten_middle(&detail, 80)));
            self.set_toast(ToastLevel::Error, message);
        }
    }

    // ---------------------------------------------------------------- palette

    fn command_palette_items(&self) -> Vec<PaletteItem> {
        use PaletteAction::Command;
        let mut items = vec![
            PaletteItem::new(
                "Link selection → DEST",
                "l · create links from SOURCE",
                "ACTION",
                Command(PaletteCommand::Link),
            ),
            PaletteItem::new(
                "Link with a custom name",
                "L / n",
                "ACTION",
                Command(PaletteCommand::LinkAs),
            ),
            PaletteItem::new(
                "Retarget DEST link to SOURCE",
                "t · change where a symlink points",
                "ACTION",
                Command(PaletteCommand::Retarget),
            ),
            PaletteItem::new(
                "Remove symlinks",
                "x · never touches targets",
                "ACTION",
                Command(PaletteCommand::Remove),
            ),
            PaletteItem::new(
                "Undo last operation",
                "u / Ctrl+Z · link, retarget or remove",
                "ACTION",
                Command(PaletteCommand::Undo),
            ),
            PaletteItem::new(
                "Redo undone operation",
                "U / Ctrl+Y · reapply safely",
                "ACTION",
                Command(PaletteCommand::Redo),
            ),
            PaletteItem::new(
                "Toggle relative links",
                format!("r · currently {}", if self.relative { "on" } else { "off" }),
                "ACTION",
                Command(PaletteCommand::ToggleRelative),
            ),
            PaletteItem::new(
                "Toggle force overwrite",
                format!("f · currently {}", if self.force { "on" } else { "off" }),
                "ACTION",
                Command(PaletteCommand::ToggleForce),
            ),
            PaletteItem::new(
                "Toggle hidden files",
                ". · show dotfiles",
                "ACTION",
                Command(PaletteCommand::ToggleHidden),
            ),
            PaletteItem::new(
                "Filter names",
                "/ · narrow the active pane",
                "ACTION",
                Command(PaletteCommand::Filter),
            ),
            PaletteItem::new(
                "Jump to path",
                "Ctrl+L",
                "ACTION",
                Command(PaletteCommand::GoTo),
            ),
            PaletteItem::new(
                "Open folder",
                "Enter · selected item",
                "ACTION",
                Command(PaletteCommand::OpenSelected),
            ),
            PaletteItem::new(
                "Switch pane",
                "Tab",
                "ACTION",
                Command(PaletteCommand::SwitchPane),
            ),
            PaletteItem::new(
                "Mark / unmark",
                "Space",
                "ACTION",
                Command(PaletteCommand::Mark),
            ),
            PaletteItem::new(
                "Mark all items",
                "a",
                "ACTION",
                Command(PaletteCommand::MarkAll),
            ),
            PaletteItem::new(
                "Clear marks",
                "c / Esc",
                "ACTION",
                Command(PaletteCommand::ClearMarks),
            ),
            PaletteItem::new(
                "Go home",
                "~ · active pane",
                "ACTION",
                Command(PaletteCommand::Home),
            ),
            PaletteItem::new(
                "Refresh panes",
                "R",
                "ACTION",
                Command(PaletteCommand::Refresh),
            ),
            PaletteItem::new(
                "Next theme",
                format!("palette · currently {}", theme::name()),
                "LOOK",
                Command(PaletteCommand::CycleTheme),
            ),
            PaletteItem::new(
                "Show keys & shortcuts",
                "?",
                "ACTION",
                Command(PaletteCommand::Help),
            ),
            PaletteItem::new(
                "Quit linkman",
                "q / Ctrl+C",
                "ACTION",
                Command(PaletteCommand::Quit),
            ),
        ];

        // Both panes often show the same folder; list each path once.
        let mut seen = std::collections::HashSet::new();
        for (pane_index, pane) in self.panes.iter().enumerate() {
            let group = if pane_index == 0 { "SOURCE" } else { "DEST" };
            for entry in &pane.entries {
                if entry.is_parent() || !seen.insert(entry.path.clone()) {
                    continue;
                }
                let mut name = entry.name.clone();
                if entry.is_dir {
                    name.push('/');
                }
                items.push(PaletteItem::new(
                    name,
                    format!("{} · {}", group, util::tildify(&entry.path)),
                    group,
                    PaletteAction::Reveal {
                        pane: pane_index,
                        path: entry.path.clone(),
                        is_dir: entry.is_dir,
                    },
                ));
            }
        }
        items
    }

    fn cycle_theme(&mut self) {
        let name = theme::cycle();
        self.set_toast(ToastLevel::Info, format!("theme · {name}"));
    }

    fn open_palette(&mut self) {
        let state = PaletteState::new(self.command_palette_items());
        self.mode = Mode::Palette(state);
    }

    fn toggle_palette(&mut self) {
        if matches!(self.mode, Mode::Palette(_)) {
            self.mode = Mode::Normal;
        } else {
            self.open_palette();
        }
    }

    fn handle_palette_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => {
                self.run_palette_selection();
                return;
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                return;
            }
            KeyCode::Up => {
                if let Mode::Palette(state) = &mut self.mode {
                    state.move_selection(-1);
                }
                return;
            }
            KeyCode::Down => {
                if let Mode::Palette(state) = &mut self.mode {
                    state.move_selection(1);
                }
                return;
            }
            KeyCode::PageUp => {
                if let Mode::Palette(state) = &mut self.mode {
                    state.move_selection(-8);
                }
                return;
            }
            KeyCode::PageDown => {
                if let Mode::Palette(state) = &mut self.mode {
                    state.move_selection(8);
                }
                return;
            }
            KeyCode::Home if ctrl => {
                if let Mode::Palette(state) = &mut self.mode {
                    state.select_first();
                }
                return;
            }
            KeyCode::End if ctrl => {
                if let Mode::Palette(state) = &mut self.mode {
                    state.select_last();
                }
                return;
            }
            _ => {}
        }

        let Mode::Palette(state) = &mut self.mode else {
            return;
        };
        match key.code {
            KeyCode::Backspace => state.input.backspace(),
            KeyCode::Delete => state.input.delete(),
            KeyCode::Left => state.input.left(),
            KeyCode::Right => state.input.right(),
            KeyCode::Home => state.input.home(),
            KeyCode::End => state.input.end(),
            KeyCode::Char('u') if ctrl => state.input.clear(),
            KeyCode::Char('w') if ctrl => state.input.delete_word(),
            KeyCode::Char('a') if ctrl => state.input.home(),
            KeyCode::Char('e') if ctrl => state.input.end(),
            KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                state.input.insert(c);
            }
            _ => return,
        }
        state.refilter();
    }

    fn run_palette_selection(&mut self) {
        self.pick_palette_index(None);
    }

    fn pick_palette_index(&mut self, index: Option<usize>) {
        let action = match &self.mode {
            Mode::Palette(state) => match index {
                Some(index) => state
                    .filtered
                    .get(index)
                    .and_then(|item| state.items.get(*item))
                    .map(|item| item.action.clone()),
                None => state.selected_action(),
            },
            _ => None,
        };
        let Some(action) = action else {
            return;
        };
        self.mode = Mode::Normal;
        match action {
            PaletteAction::Command(command) => self.run_palette_command(command),
            PaletteAction::Reveal { pane, path, is_dir } => self.reveal(pane, &path, is_dir),
        }
    }

    fn run_palette_command(&mut self, command: PaletteCommand) {
        match command {
            PaletteCommand::Link => self.link_action(),
            PaletteCommand::LinkAs => self.prompt_custom_name(),
            PaletteCommand::Retarget => self.retarget(),
            PaletteCommand::Remove => self.confirm_remove(),
            PaletteCommand::Undo => self.undo_last(),
            PaletteCommand::Redo => self.redo_last(),
            PaletteCommand::CycleTheme => self.cycle_theme(),
            PaletteCommand::ToggleRelative => self.toolbar_action(ToolbarAction::ToggleRelative),
            PaletteCommand::ToggleForce => self.toolbar_action(ToolbarAction::ToggleForce),
            PaletteCommand::ToggleHidden => self.toolbar_action(ToolbarAction::ToggleHidden),
            PaletteCommand::Refresh => {
                self.reload_all();
                self.set_toast(ToastLevel::Info, "refreshed");
            }
            PaletteCommand::Filter => self.open_prompt(PromptKind::Filter(self.active)),
            PaletteCommand::GoTo => self.open_prompt(PromptKind::GoTo(self.active)),
            PaletteCommand::Home => self.go_home(),
            PaletteCommand::SwitchPane => self.active = 1 - self.active,
            PaletteCommand::OpenSelected => self.open_selected(),
            PaletteCommand::Mark => self.toggle_mark(),
            PaletteCommand::MarkAll => self.mark_all(),
            PaletteCommand::ClearMarks => self.clear_marks(),
            PaletteCommand::Help => self.mode = Mode::Help,
            PaletteCommand::Quit => self.quit = true,
        }
    }

    fn reveal(&mut self, pane: usize, path: &Path, is_dir: bool) {
        if fs::symlink_metadata(path).is_err() {
            self.set_toast(ToastLevel::Error, "that entry is no longer there");
            return;
        }
        self.active = pane;
        if is_dir {
            self.panes[pane].cwd = path.to_path_buf();
            self.panes[pane].filter.clear();
            self.load_pane(pane, false);
        } else {
            let Some(parent) = path.parent() else {
                return;
            };
            self.panes[pane].cwd = parent.to_path_buf();
            self.panes[pane].filter.clear();
            self.load_pane(pane, false);
            self.select_path_in(pane, path);
        }
    }

    // ---------------------------------------------------------------- prompts

    fn open_prompt(&mut self, kind: PromptKind) {
        let (title, message, hint, default) = match &kind {
            PromptKind::Filter(index) => {
                let pane = &self.panes[*index];
                (
                    format!("Filter {}", if *index == 0 { "SOURCE" } else { "DEST" }),
                    format!(
                        "{} items in {}",
                        pane.visible_count(),
                        util::shorten_path(&pane.cwd, 40)
                    ),
                    "Enter apply · Esc cancel".to_string(),
                    pane.filter.clone(),
                )
            }
            PromptKind::GoTo(index) => (
                format!("Jump {}", if *index == 0 { "SOURCE" } else { "DEST" }),
                "Type a folder, or a file to reveal it".to_string(),
                "Enter open · ~ is home · Esc cancel".to_string(),
                util::tildify(&self.panes[*index].cwd),
            ),
            PromptKind::LinkName(source) => {
                let name = source
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (
                    "Link as".to_string(),
                    format!(
                        "{}  →  {}",
                        name,
                        util::shorten_path(&self.panes[1].cwd, 42)
                    ),
                    "Enter create · Esc cancel".to_string(),
                    name,
                )
            }
        };
        self.mode = Mode::Prompt(PromptState {
            title,
            message,
            hint,
            input: TextInput::new(&default),
            kind,
        });
    }

    fn submit_prompt(&mut self) {
        let Mode::Prompt(prompt) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            return;
        };
        match prompt.kind {
            PromptKind::Filter(index) => {
                self.panes[index].filter = prompt.input.value().trim().to_string();
                self.panes[index].selected = 0;
            }
            PromptKind::GoTo(index) => self.goto(index, prompt.input.value()),
            PromptKind::LinkName(source) => {
                let name = prompt.input.value().trim().to_string();
                if name.is_empty() || name == "." || name == ".." || name.contains('/') {
                    self.set_toast(ToastLevel::Error, format!("invalid link name: {name:?}"));
                    return;
                }
                let dest = self.panes[1].cwd.clone();
                self.start_link(vec![source], dest, vec![name]);
            }
        }
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                self.submit_prompt();
                return;
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                return;
            }
            _ => {}
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Mode::Prompt(prompt) = &mut self.mode else {
            return;
        };
        match key.code {
            KeyCode::Backspace => prompt.input.backspace(),
            KeyCode::Delete => prompt.input.delete(),
            KeyCode::Left => prompt.input.left(),
            KeyCode::Right => prompt.input.right(),
            KeyCode::Home => prompt.input.home(),
            KeyCode::End => prompt.input.end(),
            KeyCode::Char('u') if ctrl => prompt.input.clear(),
            KeyCode::Char('w') if ctrl => prompt.input.delete_word(),
            KeyCode::Char('a') if ctrl => prompt.input.home(),
            KeyCode::Char('e') if ctrl => prompt.input.end(),
            KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                prompt.input.insert(c)
            }
            _ => {}
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => self.confirm_yes(),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.mode = Mode::Normal,
            _ => {}
        }
    }

    fn confirm_yes(&mut self) {
        let Mode::Confirm(confirm) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            return;
        };
        match confirm.action {
            ConfirmAction::RemoveLinks(paths) => self.do_remove(paths),
        }
    }

    fn handle_conflict_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Mode::Conflict(conflict) = &mut self.mode {
                    conflict.choice = conflict.choice.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Mode::Conflict(conflict) = &mut self.mode {
                    conflict.choice = (conflict.choice + 1).min(3);
                }
            }
            KeyCode::Char('o') => self.resolve_conflict(0),
            KeyCode::Char('b') => self.resolve_conflict(1),
            KeyCode::Char('s') => self.resolve_conflict(2),
            KeyCode::Char('c') | KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                let choice = match &self.mode {
                    Mode::Conflict(conflict) => conflict.choice,
                    _ => return,
                };
                self.resolve_conflict(choice);
            }
            _ => {}
        }
    }

    fn resolve_conflict(&mut self, choice: usize) {
        let Mode::Conflict(conflict) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            return;
        };
        match choice {
            0 => self.run_link(
                conflict.sources,
                conflict.dest,
                conflict.names,
                true,
                false,
                false,
            ),
            1 => self.run_link(
                conflict.sources,
                conflict.dest,
                conflict.names,
                false,
                true,
                false,
            ),
            2 => self.run_link(
                conflict.sources,
                conflict.dest,
                conflict.names,
                false,
                false,
                true,
            ),
            _ => {}
        }
    }

    // ------------------------------------------------------------------ keys

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.kind == ratatui::crossterm::event::KeyEventKind::Release {
            return Ok(());
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return Ok(());
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('p') {
            if matches!(
                self.mode,
                Mode::Prompt(_) | Mode::Confirm(_) | Mode::Conflict(_)
            ) {
                return Ok(());
            }
            self.toggle_palette();
            return Ok(());
        }
        match self.mode {
            Mode::Help => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter
                ) {
                    self.mode = Mode::Normal;
                }
                return Ok(());
            }
            Mode::Prompt(_) => {
                self.handle_prompt_key(key);
                return Ok(());
            }
            Mode::Confirm(_) => {
                self.handle_confirm_key(key);
                return Ok(());
            }
            Mode::Conflict(_) => {
                self.handle_conflict_key(key);
                return Ok(());
            }
            Mode::Palette(_) => {
                self.handle_palette_key(key);
                return Ok(());
            }
            Mode::Normal => {}
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => self.quit = true,
                KeyCode::Char('l') => self.open_prompt(PromptKind::GoTo(self.active)),
                KeyCode::Char('r') => {
                    self.reload_all();
                    self.set_toast(ToastLevel::Info, "refreshed");
                }
                KeyCode::Char('d') => self.move_selection(8),
                KeyCode::Char('u') => self.move_selection(-8),
                KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.redo_last()
                }
                KeyCode::Char('Z') => self.redo_last(),
                KeyCode::Char('z') => self.undo_last(),
                KeyCode::Char('y') => self.redo_last(),
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Tab | KeyCode::BackTab => self.active = 1 - self.active,
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::Home | KeyCode::Char('g') => self.panes[self.active].selected = 0,
            KeyCode::End | KeyCode::Char('G') => {
                let count = self.panes[self.active].visible_count();
                self.panes[self.active].selected = count.saturating_sub(1);
            }
            KeyCode::Enter | KeyCode::Right => self.open_selected(),
            KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h') => self.go_parent(),
            KeyCode::Char(' ') => self.toggle_mark(),
            KeyCode::Char('a') => self.mark_all(),
            KeyCode::Char('c') => self.clear_marks(),
            KeyCode::Char('r') => {
                self.relative = !self.relative;
                let message = if self.relative {
                    "relative links on"
                } else {
                    "absolute links on"
                };
                self.set_toast(ToastLevel::Info, message);
            }
            KeyCode::Char('f') => {
                self.force = !self.force;
                let message = if self.force {
                    "force on — existing entries will be replaced"
                } else {
                    "force off — conflicts will ask first"
                };
                self.set_toast(ToastLevel::Info, message);
            }
            KeyCode::Char('.') => {
                self.show_hidden = !self.show_hidden;
                self.reload_all();
                let message = if self.show_hidden {
                    "showing hidden files"
                } else {
                    "hiding dotfiles"
                };
                self.set_toast(ToastLevel::Info, message);
            }
            KeyCode::Char('R') => {
                self.reload_all();
                self.set_toast(ToastLevel::Info, "refreshed");
            }
            KeyCode::Char('l') => self.link_action(),
            KeyCode::Char('L') | KeyCode::Char('n') => self.prompt_custom_name(),
            KeyCode::Char('x') | KeyCode::Delete => self.confirm_remove(),
            KeyCode::Char('t') => self.retarget(),
            KeyCode::Char('u') => self.undo_last(),
            KeyCode::Char('U') => self.redo_last(),
            KeyCode::Char('/') => self.open_prompt(PromptKind::Filter(self.active)),
            KeyCode::Char('~') => self.go_home(),
            KeyCode::Char('1') => self.active = 0,
            KeyCode::Char('2') => self.active = 1,
            KeyCode::Esc => self.escape(),
            _ => {}
        }
        Ok(())
    }

    fn escape(&mut self) {
        if !self.panes[self.active].filter.is_empty() {
            self.panes[self.active].filter.clear();
            self.panes[self.active].selected = 0;
            self.set_toast(ToastLevel::Info, "filter cleared");
        } else if !self.marks.is_empty() {
            self.marks.clear();
            self.set_toast(ToastLevel::Info, "marks cleared");
        }
    }

    pub fn handle_paste(&mut self, text: String) {
        match &mut self.mode {
            Mode::Prompt(prompt) => prompt.input.insert_str(&text),
            Mode::Palette(state) => {
                state.input.insert_str(&text);
                state.refilter();
            }
            _ => {}
        }
    }

    // ----------------------------------------------------------------- mouse

    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if !matches!(self.mode, Mode::Normal) {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let action = self
                        .map
                        .modal
                        .iter()
                        .find(|(rect, _)| inside(*rect, mouse.column, mouse.row))
                        .map(|(_, action)| *action);
                    if let Some(action) = action {
                        self.modal_action(action);
                    }
                }
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                    if matches!(self.mode, Mode::Palette(_)) {
                        let delta = if mouse.kind == MouseEventKind::ScrollUp {
                            -3
                        } else {
                            3
                        };
                        if let Mode::Palette(state) = &mut self.mode {
                            state.move_selection(delta);
                        }
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(action) = self
                    .map
                    .toolbar
                    .iter()
                    .find(|(rect, _)| inside(*rect, mouse.column, mouse.row))
                    .map(|(_, action)| *action)
                {
                    self.toolbar_action(action);
                    return Ok(());
                }

                if let Some(hit) = self
                    .map
                    .rows
                    .iter()
                    .find(|hit| inside(hit.area, mouse.column, mouse.row))
                    .copied()
                {
                    let double_click = self
                        .last_click
                        .map(|(when, col, row)| {
                            col == mouse.column
                                && row == mouse.row
                                && when.elapsed() < Duration::from_millis(450)
                        })
                        .unwrap_or(false);
                    self.last_click = Some((Instant::now(), mouse.column, mouse.row));
                    self.active = hit.pane;
                    self.panes[hit.pane].selected = hit.row;
                    if double_click {
                        self.open_selected();
                    }
                    return Ok(());
                }

                for index in 0..2 {
                    if inside(self.map.panes[index], mouse.column, mouse.row) {
                        self.active = index;
                        break;
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(hit) = self
                    .map
                    .rows
                    .iter()
                    .find(|hit| inside(hit.area, mouse.column, mouse.row))
                    .copied()
                {
                    self.active = hit.pane;
                    self.panes[hit.pane].selected = hit.row;
                    self.toggle_mark();
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let step = if mouse.kind == MouseEventKind::ScrollUp {
                    -3
                } else {
                    3
                };
                for index in 0..2 {
                    if inside(self.map.panes[index], mouse.column, mouse.row) {
                        self.move_pane_selection(index, step);
                        break;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn toolbar_action(&mut self, action: ToolbarAction) {
        match action {
            ToolbarAction::Link => self.link_action(),
            ToolbarAction::Rename => self.prompt_custom_name(),
            ToolbarAction::ToggleRelative => {
                self.relative = !self.relative;
                let message = if self.relative {
                    "relative links on"
                } else {
                    "absolute links on"
                };
                self.set_toast(ToastLevel::Info, message);
            }
            ToolbarAction::ToggleForce => {
                self.force = !self.force;
                let message = if self.force {
                    "force on — existing entries will be replaced"
                } else {
                    "force off — conflicts will ask first"
                };
                self.set_toast(ToastLevel::Info, message);
            }
            ToolbarAction::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.reload_all();
            }
            ToolbarAction::Help => self.mode = Mode::Help,
        }
    }

    fn modal_action(&mut self, action: ModalAction) {
        match action {
            ModalAction::PromptSubmit => self.submit_prompt(),
            ModalAction::PromptCancel => self.mode = Mode::Normal,
            ModalAction::ConfirmYes => self.confirm_yes(),
            ModalAction::ConfirmNo => self.mode = Mode::Normal,
            ModalAction::ConflictChoice(choice) => self.resolve_conflict(choice),
            ModalAction::CloseHelp => self.mode = Mode::Normal,
            ModalAction::PalettePick(index) => self.pick_palette_index(Some(index)),
        }
    }

    // ------------------------------------------------------------- ui helpers

    pub fn status_badges(&self) -> Vec<String> {
        let mut badges = Vec::new();
        badges.push(if self.relative {
            "REL".into()
        } else {
            "ABS".into()
        });
        if self.force {
            badges.push("FORCE".into());
        }
        if self.show_hidden {
            badges.push("HIDDEN".into());
        }
        if !self.marks.is_empty() {
            badges.push(format!("{} MARKED", self.marks.len()));
        }
        badges
    }

    pub fn active_entry(&self) -> Option<&Entry> {
        self.panes[self.active].selected_entry()
    }

    pub fn shortcut_hints(&self) -> Vec<(String, String)> {
        match self.mode {
            Mode::Normal => vec![
                ("^P".into(), "find".into()),
                ("?".into(), "help".into()),
                ("Tab".into(), "pane".into()),
                ("Space".into(), "mark".into()),
                ("l".into(), "link".into()),
                ("u".into(), "undo".into()),
                ("U".into(), "redo".into()),
                ("x".into(), "unlink".into()),
                ("/".into(), "filter".into()),
            ],
            Mode::Help => vec![("Esc".into(), "close".into())],
            Mode::Prompt(_) => vec![
                ("Enter".into(), "confirm".into()),
                ("Esc".into(), "cancel".into()),
                ("^U".into(), "clear".into()),
            ],
            Mode::Confirm(_) => vec![("y".into(), "remove".into()), ("n".into(), "cancel".into())],
            Mode::Conflict(_) => vec![
                ("↑↓".into(), "choose".into()),
                ("Enter".into(), "apply".into()),
                ("Esc".into(), "cancel".into()),
            ],
            Mode::Palette(_) => vec![
                ("↑↓".into(), "move".into()),
                ("Enter".into(), "run".into()),
                ("Esc".into(), "close".into()),
            ],
        }
    }
}

fn inside(rect: ratatui::layout::Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::ui::RowHit;
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

    fn fixture() -> (App, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::create_dir(source.join("adir")).unwrap();
        fs::write(source.join("afile"), "hello").unwrap();
        let dest = tmp.path().join("dest");
        fs::create_dir(&dest).unwrap();
        let app = App::new(source, dest, &Config::default());
        (app, tmp)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn select(app: &mut App, pane: usize, name: &str) {
        let index = app.panes[pane]
            .entries
            .iter()
            .position(|entry| entry.name == name)
            .unwrap();
        let position = app.panes[pane]
            .visible_indices()
            .iter()
            .position(|entry| *entry == index)
            .unwrap();
        app.panes[pane].selected = position;
    }

    fn click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn mouse_selects_opens_and_scrolls() {
        let (mut app, _tmp) = fixture();
        app.map.panes = [
            ratatui::layout::Rect::new(0, 5, 50, 20),
            ratatui::layout::Rect::new(51, 5, 50, 20),
        ];
        app.map.rows = vec![
            RowHit {
                pane: 0,
                row: 0,
                area: ratatui::layout::Rect::new(0, 8, 50, 1),
            },
            RowHit {
                pane: 0,
                row: 1,
                area: ratatui::layout::Rect::new(0, 9, 50, 1),
            },
            RowHit {
                pane: 0,
                row: 2,
                area: ratatui::layout::Rect::new(0, 10, 50, 1),
            },
        ];

        app.handle_mouse(click(5, 10)).unwrap();
        assert_eq!(app.active, 0);
        assert_eq!(app.panes[0].selected, 2);

        let wheel = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 5,
            row: 6,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(wheel).unwrap();
        assert_eq!(app.panes[0].selected, 0);

        // Two clicks on the same folder row open it.
        app.handle_mouse(click(5, 9)).unwrap();
        app.handle_mouse(click(5, 9)).unwrap();
        assert!(app.panes[0].cwd.ends_with("adir"));

        app.map.toolbar = vec![(
            ratatui::layout::Rect::new(0, 1, 8, 1),
            ToolbarAction::ToggleRelative,
        )];
        app.handle_mouse(click(2, 1)).unwrap();
        assert!(app.relative);
    }

    #[test]
    fn links_and_handles_conflicts() {
        let (mut app, _tmp) = fixture();
        app.handle_key(key(KeyCode::Char('l'))).unwrap();
        let link = app.panes[1].cwd.join("adir");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );

        app.handle_key(key(KeyCode::Char('l'))).unwrap();
        assert!(matches!(app.mode, Mode::Conflict(_)));
        app.handle_key(key(KeyCode::Char('s'))).unwrap();
        assert!(matches!(app.mode, Mode::Normal));
        assert!(fs::symlink_metadata(&link).is_ok());

        app.active = 1;
        app.panes[1].selected = 1;
        app.handle_key(key(KeyCode::Char('x'))).unwrap();
        assert!(matches!(app.mode, Mode::Confirm(_)));
        app.handle_key(key(KeyCode::Char('y'))).unwrap();
        assert!(fs::symlink_metadata(&link).is_err());
    }

    #[test]
    fn conflict_can_back_up_the_old_entry() {
        let (mut app, _tmp) = fixture();
        let link = app.panes[1].cwd.join("adir");
        fs::write(&link, "old").unwrap();

        app.handle_key(key(KeyCode::Char('l'))).unwrap();
        assert!(matches!(app.mode, Mode::Conflict(_)));
        app.handle_key(key(KeyCode::Char('b'))).unwrap();

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let backup = app.panes[1].cwd.join("adir.bak");
        assert_eq!(fs::read_to_string(backup).unwrap(), "old");
    }

    #[test]
    fn undo_covers_link_remove_and_retarget() {
        let (mut app, _tmp) = fixture();
        let link = app.panes[1].cwd.join("adir");
        app.handle_key(key(KeyCode::Char('l'))).unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        app.handle_key(ctrl('z')).unwrap();
        assert!(fs::symlink_metadata(&link).is_err());

        app.handle_key(key(KeyCode::Char('l'))).unwrap();
        app.active = 1;
        app.panes[1].selected = 1;
        app.handle_key(key(KeyCode::Char('x'))).unwrap();
        app.handle_key(key(KeyCode::Char('y'))).unwrap();
        assert!(fs::symlink_metadata(&link).is_err());
        app.handle_key(key(KeyCode::Char('u'))).unwrap();
        assert!(fs::symlink_metadata(&link).is_ok());

        app.active = 0;
        app.panes[0].selected = 2; // afile
        app.active = 1;
        app.panes[1].selected = 1;
        app.handle_key(key(KeyCode::Char('t'))).unwrap();
        assert!(fs::read_link(&link).unwrap().ends_with("afile"));
        app.handle_key(key(KeyCode::Char('u'))).unwrap();
        assert!(fs::read_link(&link).unwrap().ends_with("adir"));
    }

    #[test]
    fn redo_reapplies_and_is_consumed() {
        let (mut app, _tmp) = fixture();
        let link = app.panes[1].cwd.join("adir");
        app.handle_key(key(KeyCode::Char('l'))).unwrap();
        app.handle_key(ctrl('z')).unwrap();
        assert!(fs::symlink_metadata(&link).is_err());

        app.handle_key(ctrl('y')).unwrap();
        assert!(fs::symlink_metadata(&link).is_ok());
        // Redo is one-shot: nothing is left after it ran.
        app.handle_key(ctrl('y')).unwrap();
        assert!(fs::symlink_metadata(&link).is_ok());

        app.active = 1;
        select(&mut app, 1, "adir");
        app.handle_key(key(KeyCode::Char('x'))).unwrap();
        app.handle_key(key(KeyCode::Char('y'))).unwrap();
        assert!(fs::symlink_metadata(&link).is_err());
        app.handle_key(key(KeyCode::Char('u'))).unwrap();
        assert!(fs::symlink_metadata(&link).is_ok());
        app.handle_key(key(KeyCode::Char('U'))).unwrap();
        assert!(fs::symlink_metadata(&link).is_err());
    }

    #[test]
    fn a_new_action_drops_redo_history() {
        let (mut app, _tmp) = fixture();
        let link = app.panes[1].cwd.join("adir");
        app.handle_key(key(KeyCode::Char('l'))).unwrap();
        app.handle_key(ctrl('z')).unwrap();
        assert!(fs::symlink_metadata(&link).is_err());

        app.handle_key(key(KeyCode::Char('l'))).unwrap();
        app.handle_key(ctrl('y')).unwrap();
        assert!(fs::symlink_metadata(&link).is_ok());
    }

    #[test]
    fn palette_finds_actions_and_reveals_files() {
        let (mut app, _tmp) = fixture();
        app.handle_key(ctrl('p')).unwrap();
        assert!(matches!(app.mode, Mode::Palette(_)));

        for c in "toggle rel".chars() {
            app.handle_key(key(KeyCode::Char(c))).unwrap();
        }
        app.handle_key(key(KeyCode::Enter)).unwrap();
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.relative);

        app.handle_key(ctrl('p')).unwrap();
        for c in "afile".chars() {
            app.handle_key(key(KeyCode::Char(c))).unwrap();
        }
        app.handle_key(key(KeyCode::Enter)).unwrap();
        assert_eq!(
            app.panes[0]
                .selected_entry()
                .map(|entry| entry.name.as_str()),
            Some("afile")
        );
    }

    #[test]
    fn config_sets_initial_toggles() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join(".hidden"), "x").unwrap();
        let dest = tmp.path().join("dest");
        fs::create_dir(&dest).unwrap();
        let config = Config {
            relative: true,
            force: true,
            show_hidden: true,
            undo_limit: 7,
            theme: "black".to_string(),
            source: None,
            dest: None,
        };
        let app = App::new(source, dest, &config);
        assert!(app.relative);
        assert!(app.force);
        assert!(app.show_hidden);
        assert!(
            app.panes[0]
                .entries
                .iter()
                .any(|entry| entry.name == ".hidden")
        );
        assert_eq!(app.undo_limit, 7);
    }

    #[test]
    fn relative_toggle_changes_new_links() {
        let (mut app, _tmp) = fixture();
        app.relative = true;
        app.handle_key(key(KeyCode::Char('l'))).unwrap();
        let link = app.panes[1].cwd.join("adir");
        let target = fs::read_link(&link).unwrap();
        assert!(target.is_relative());
        assert_eq!(target, PathBuf::from("../source/adir"));
    }
}
