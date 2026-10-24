//! Rendering for the linkman TUI. Everything is drawn by hand so the layout,
//! hover areas and visual rhythm stay under our control.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

use super::app::{App, ConfirmState, ConflictState, Mode, PromptKind, PromptState, ToastLevel};
use super::entry::{Entry, EntryKind};
use super::palette::PaletteState;
use super::theme::{self, *};
use crate::util;

#[derive(Debug, Clone, Copy)]
pub struct RowHit {
    pub pane: usize,
    pub row: usize,
    pub area: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    Link,
    Rename,
    ToggleRelative,
    ToggleForce,
    ToggleHidden,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalAction {
    PromptSubmit,
    PromptCancel,
    ConfirmYes,
    ConfirmNo,
    ConflictChoice(usize),
    CloseHelp,
    PalettePick(usize),
}

#[derive(Debug, Clone)]
pub struct UiMap {
    pub panes: [Rect; 2],
    pub rows: Vec<RowHit>,
    pub toolbar: Vec<(Rect, ToolbarAction)>,
    pub modal: Vec<(Rect, ModalAction)>,
}

impl UiMap {
    pub fn empty() -> Self {
        Self {
            panes: [Rect::new(0, 0, 0, 0); 2],
            rows: Vec::new(),
            toolbar: Vec::new(),
            modal: Vec::new(),
        }
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let mut map = UiMap::empty();
    let area = frame.area();
    frame.render_widget(Block::default().style(theme::base()), area);

    if area.width < 64 || area.height < 18 {
        draw_too_small(frame, area);
        app.map = map;
        return;
    }

    let inspector_height = if area.height >= 26 { 7 } else { 0 };
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(inspector_height),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(frame, app, rows[0], &mut map);

    let pane_areas = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(rows[1]);
    map.panes = [pane_areas[0], pane_areas[1]];
    draw_pane(frame, app, 0, pane_areas[0], &mut map);
    draw_pane(frame, app, 1, pane_areas[1], &mut map);

    if inspector_height > 0 {
        draw_inspector(frame, app, rows[2]);
    }
    draw_status(frame, app, rows[3]);
    draw_footer(frame, app, rows[4]);

    match &app.mode {
        Mode::Normal => {}
        Mode::Help => draw_help(frame, app, area, &mut map),
        Mode::Prompt(prompt) => draw_prompt(frame, prompt, area, &mut map),
        Mode::Confirm(confirm) => draw_confirm(frame, confirm, area, &mut map),
        Mode::Conflict(conflict) => draw_conflict(frame, conflict, area, &mut map),
        Mode::Palette(palette) => draw_palette(frame, palette, area, &mut map),
    }

    app.map = map;
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let message = Text::from(vec![
        Line::from(Span::styled(
            "linkman needs a little more room",
            Style::default().fg(amber()).add_modifier(Modifier::BOLD),
        )),
        Line::default(),
        Line::from(Span::styled(
            "resize this terminal to at least 64 × 18",
            Style::default().fg(muted()),
        )),
        Line::default(),
        Line::from(Span::styled("q quit", Style::default().fg(faint_color()))),
    ]);
    let block = Paragraph::new(message)
        .alignment(Alignment::Center)
        .style(theme::base());
    frame.render_widget(block, area);
}

// ---------------------------------------------------------------------------
// header
// ---------------------------------------------------------------------------

fn header_height_parts(area: Rect) -> [Rect; 3] {
    let parts = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    [parts[0], parts[1], parts[2]]
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect, map: &mut UiMap) {
    let rows = header_height_parts(area);

    let brand_area = Rect {
        x: area.x.saturating_add(2),
        y: rows[0].y,
        width: area.width.saturating_sub(4),
        height: 1,
    };
    let mut brand = vec![Span::styled(ICON_CURSOR, Style::default().fg(mint()))];
    brand.push(Span::raw(" "));
    brand.extend(theme::wordmark("linkman"));
    brand.push(Span::styled(
        "   symlink studio",
        Style::default().fg(faint_color()),
    ));
    frame.render_widget(Paragraph::new(Line::from(brand)), brand_area);

    let items: Vec<(&str, String, bool, ToolbarAction)> = vec![
        ("l", "link".to_string(), false, ToolbarAction::Link),
        ("L", "rename".to_string(), false, ToolbarAction::Rename),
        (
            "r",
            if app.relative { "rel on" } else { "rel off" }.to_string(),
            app.relative,
            ToolbarAction::ToggleRelative,
        ),
        (
            "f",
            if app.force { "force on" } else { "force off" }.to_string(),
            app.force,
            ToolbarAction::ToggleForce,
        ),
        (
            ".",
            if app.show_hidden {
                "hidden on"
            } else {
                "hidden off"
            }
            .to_string(),
            app.show_hidden,
            ToolbarAction::ToggleHidden,
        ),
        ("?", "help".to_string(), false, ToolbarAction::Help),
    ];

    let mut x = area.x.saturating_add(2);
    let limit = area.right().saturating_sub(1);
    for (key, label, active, action) in items {
        let width = key.chars().count() + label.chars().count() + 4;
        let width = width as u16;
        if x.saturating_add(width) > limit {
            break;
        }
        let accent = toolbar_accent(action);
        let badge_style = if active {
            Style::default()
                .fg(bg())
                .bg(accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(accent).bg(surface_hi())
        };
        let label_style = if active {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(muted())
        };
        let line = Line::from(vec![
            Span::styled(format!(" {key} "), badge_style),
            Span::styled(format!(" {label} "), label_style),
        ]);
        let rect = Rect {
            x,
            y: rows[1].y,
            width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(line), rect);
        map.toolbar.push((rect, action));
        x = x.saturating_add(width + 1);
    }

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(area.width as usize),
            Style::default().fg(border_soft()),
        ))),
        rows[2],
    );
}

fn toolbar_accent(action: ToolbarAction) -> Color {
    match action {
        ToolbarAction::ToggleForce => amber(),
        ToolbarAction::ToggleHidden => violet(),
        _ => mint(),
    }
}

// ---------------------------------------------------------------------------
// panes
// ---------------------------------------------------------------------------

fn draw_pane(frame: &mut Frame, app: &App, index: usize, area: Rect, map: &mut UiMap) {
    let pane = &app.panes[index];
    let active = app.active == index;
    let role = if index == 0 { "SOURCE" } else { "DEST" };
    let badge_style = if active {
        Style::default()
            .fg(bg())
            .bg(mint())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(muted()).bg(surface_hi())
    };

    let mut title = vec![Span::styled(format!(" {role} "), badge_style)];
    let item_count = pane
        .visible_indices()
        .into_iter()
        .filter(|index| !pane.entries[*index].is_parent())
        .count();
    title.push(Span::styled(
        format!(
            "{item_count} {} ",
            if item_count == 1 { "item" } else { "items" }
        ),
        Style::default().fg(faint_color()),
    ));
    let mut title_width = role.len() + 2 + item_count.to_string().len() + 7;
    if !pane.filter.is_empty() {
        let filter = util::truncate(&pane.filter, 14);
        title_width += filter.chars().count() + 3;
        title.push(Span::styled(
            format!(" /{filter} "),
            Style::default().fg(amber()),
        ));
    }

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if active { mint() } else { border() }))
        .style(theme::surface())
        .title(Line::from(title));
    let path_budget = (area.width as usize)
        .saturating_sub(title_width)
        .saturating_sub(6)
        .min(34);
    if path_budget >= 10 {
        let path = util::shorten_path(&pane.cwd, path_budget);
        block = block.title(
            Line::from(Span::styled(
                format!(" {path} "),
                Style::default().fg(if active { muted() } else { faint_color() }),
            ))
            .right_aligned(),
        );
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    if let Some(error) = &pane.error {
        let rect = Rect {
            x: inner.x + 2,
            y: inner.y,
            width: inner.width.saturating_sub(4),
            height: inner.height.min(2),
        };
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(Span::styled(
                    "cannot read this folder",
                    Style::default().fg(danger()).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    util::truncate(error, rect.width as usize),
                    Style::default().fg(muted()),
                )),
            ]))
            .wrap(Wrap { trim: true }),
            rect,
        );
        return;
    }

    let show_header = inner.height >= 3;
    let body_y = inner.y + if show_header { 1 } else { 0 };
    let body_height = inner.height.saturating_sub(if show_header { 1 } else { 0 });
    if body_height == 0 {
        return;
    }

    let visible = pane.visible_indices();
    let total = visible.len();
    let rows = body_height as usize;

    let right_width = visible
        .iter()
        .map(|entry| pane.entries[*entry].right_label().chars().count())
        .max()
        .unwrap_or(3)
        .clamp(3, 9);
    let scrollbar = total > rows;
    let content_width = inner.width.saturating_sub(if scrollbar { 1 } else { 0 });
    if content_width < 16 {
        return;
    }
    let name_width = (content_width as usize)
        .saturating_sub(4 + 2 + right_width + 1)
        .max(3);

    if show_header {
        let header_y = inner.y;
        let name_rect = Rect {
            x: inner.x + 4,
            y: header_y,
            width: content_width.saturating_sub(4),
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Span::styled("NAME", theme::faint())),
            name_rect,
        );
        let info_rect = Rect {
            x: inner.x,
            y: header_y,
            width: content_width.saturating_sub(1),
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("INFO", theme::faint())).right_aligned()),
            info_rect,
        );
    }

    if total == 0 {
        let rect = Rect {
            x: inner.x + 2,
            y: body_y,
            width: inner.width.saturating_sub(4),
            height: body_height,
        };
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::default(),
                Line::from(Span::styled(
                    "this folder is empty",
                    Style::default().fg(muted()),
                )),
                Line::from(Span::styled(
                    "← go up and pick another",
                    Style::default().fg(faint_color()),
                )),
            ]))
            .alignment(Alignment::Center),
            rect,
        );
        return;
    }

    let offset = if pane.selected >= rows {
        pane.selected + 1 - rows
    } else {
        0
    };

    for row in 0..rows {
        let position = offset + row;
        if position >= total {
            break;
        }
        let entry = &pane.entries[visible[position]];
        let selected = position == pane.selected;
        let marked = app.marks.contains(&entry.path);
        let line = entry_line(entry, selected, marked, active, name_width, right_width);
        let row_style = Style::default().bg(if selected {
            if active { select_bg() } else { surface_hi() }
        } else {
            surface_color()
        });
        let rect = Rect {
            x: inner.x,
            y: body_y + row as u16,
            width: inner.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(line).style(row_style), rect);
        map.rows.push(RowHit {
            pane: index,
            row: position,
            area: rect,
        });
    }

    if total <= 1 && body_height > 2 {
        let hint = if !pane.filter.is_empty() {
            "no names match this filter"
        } else if index == 0 {
            "this folder is empty"
        } else {
            "created links land here"
        };
        frame.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(faint_color()))),
            Rect {
                x: inner.x + 4,
                y: body_y + 2,
                width: inner.width.saturating_sub(6),
                height: 1,
            },
        );
    }

    if scrollbar {
        let track_x = inner.right().saturating_sub(1);
        let thumb_height = ((rows * rows) / total).max(1).min(rows);
        let max_offset = total.saturating_sub(rows).max(1);
        let thumb_top = offset * (rows.saturating_sub(thumb_height)) / max_offset;
        for row in 0..rows {
            let is_thumb = row >= thumb_top && row < thumb_top + thumb_height;
            let (symbol, color) = if is_thumb {
                ("┃", mint_dim())
            } else {
                ("│", border())
            };
            let rect = Rect {
                x: track_x,
                y: body_y + row as u16,
                width: 1,
                height: 1,
            };
            frame.render_widget(
                Paragraph::new(symbol).style(Style::default().fg(color).bg(surface_color())),
                rect,
            );
        }
    }
}

fn entry_line(
    entry: &Entry,
    selected: bool,
    marked: bool,
    active: bool,
    name_width: usize,
    right_width: usize,
) -> Line<'static> {
    let (glyph, color) = match entry.kind {
        EntryKind::Parent => (ICON_PARENT, muted()),
        EntryKind::Dir => (ICON_DIR, blue()),
        EntryKind::File => (ICON_FILE, faint_color()),
        EntryKind::Link => (ICON_LINK, amber()),
        EntryKind::Broken => (ICON_BROKEN, danger()),
        EntryKind::Loop => (ICON_LOOP, violet()),
    };

    let bar_color = if selected && active {
        mint()
    } else {
        faint_color()
    };
    let bar = if selected { ICON_CURSOR } else { " " };
    let mark = if marked { ICON_MARK } else { " " };

    let mut name = entry.name.clone();
    if entry.is_dir && !entry.is_parent() {
        name.push('/');
    }
    let name = util::truncate(&name, name_width);
    let name_padding = " ".repeat(name_width.saturating_sub(name.chars().count()));

    let name_color = if selected {
        bright()
    } else if entry.hidden {
        faint_color()
    } else {
        color
    };
    let name_style = if selected {
        Style::default().fg(name_color).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(name_color)
    };

    let right = entry.right_label();
    let right_color = if selected {
        text_color()
    } else {
        match entry.kind {
            EntryKind::Broken | EntryKind::Loop => danger(),
            EntryKind::Link => amber_dim(),
            EntryKind::Dir => faint_color(),
            _ => muted(),
        }
    };
    let right_padded = format!(
        "{}{}",
        " ".repeat(right_width.saturating_sub(right.chars().count())),
        right
    );

    Line::from(vec![
        Span::styled(bar, Style::default().fg(bar_color)),
        Span::styled(mark, Style::default().fg(amber())),
        Span::styled(format!("{glyph} "), Style::default().fg(color)),
        Span::styled(name, name_style),
        Span::raw(name_padding),
        Span::raw("  "),
        Span::styled(right_padded, Style::default().fg(right_color)),
        Span::raw(" "),
    ])
}

// ---------------------------------------------------------------------------
// inspector
// ---------------------------------------------------------------------------

fn draw_inspector(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border()))
        .style(theme::surface())
        .title(Line::from(Span::styled(
            " INSPECTOR ",
            Style::default().fg(muted()).add_modifier(Modifier::BOLD),
        )))
        .title(
            Line::from(Span::styled(
                " r relative · f force · Space mark ",
                Style::default().fg(faint_color()),
            ))
            .right_aligned(),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let width = inner.width.saturating_sub(3) as usize;

    let lines: Vec<Line> = match app.active_entry() {
        None => vec![Line::from(Span::styled(
            "nothing selected",
            Style::default().fg(muted()),
        ))],
        Some(entry) => {
            let color = kind_color(entry);
            let mut first = vec![
                Span::styled(
                    format!("{} ", kind_glyph(entry)),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    util::truncate(&entry.name, width.saturating_sub(24)),
                    Style::default().fg(bright()).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    format!(" {} ", kind_word(entry)),
                    Style::default().fg(color).bg(surface_hi()),
                ),
                Span::styled(
                    format!("  {}", entry.right_label()),
                    Style::default().fg(muted()),
                ),
            ];
            if app.marks.contains(&entry.path) {
                first.push(Span::styled(
                    format!("   {ICON_MARK} marked"),
                    Style::default().fg(amber()),
                ));
            }

            let second = vec![Span::styled(
                util::truncate(&util::tildify(&entry.path), width),
                Style::default().fg(muted()),
            )];

            let third = if entry.is_link() {
                let mut spans = vec![Span::styled(
                    format!("{ICON_LINK} "),
                    Style::default().fg(amber()),
                )];
                spans.push(Span::styled(
                    util::truncate(
                        &entry
                            .target
                            .as_ref()
                            .map(|target| target.display().to_string())
                            .unwrap_or_else(|| "?".to_string()),
                        width.saturating_sub(30),
                    ),
                    Style::default().fg(amber()),
                ));
                spans.push(match entry.kind {
                    EntryKind::Broken => Span::styled(
                        "   ✗ broken",
                        Style::default().fg(danger()).add_modifier(Modifier::BOLD),
                    ),
                    EntryKind::Loop => Span::styled(
                        "   ↻ loop",
                        Style::default().fg(danger()).add_modifier(Modifier::BOLD),
                    ),
                    _ => Span::styled("   ✓ resolves", Style::default().fg(mint())),
                });
                if let Some(resolved) = &entry.resolved {
                    spans.push(Span::styled(
                        format!(
                            "   {}",
                            util::shorten_path(resolved, width.saturating_sub(40).max(8))
                        ),
                        Style::default().fg(faint_color()),
                    ));
                }
                Line::from(spans)
            } else {
                let mut spans = vec![
                    Span::styled(kind_word(entry), Style::default().fg(color)),
                    Span::styled(
                        format!(
                            "   ·   {}",
                            entry.mode_string().unwrap_or_else(|| "—".to_string())
                        ),
                        Style::default().fg(muted()),
                    ),
                    Span::styled(
                        format!("   ·   {}", util::ago_label(entry.modified)),
                        Style::default().fg(faint_color()),
                    ),
                ];
                if entry.hidden {
                    spans.push(Span::styled("   ·   hidden", Style::default().fg(violet())));
                }
                Line::from(spans)
            };

            let mut out = vec![Line::from(first), Line::from(second), third];
            if entry.is_link() {
                let mut fourth = vec![
                    Span::styled(
                        entry.mode_string().unwrap_or_else(|| "—".to_string()),
                        Style::default().fg(faint_color()),
                    ),
                    Span::styled(
                        format!("   ·   modified {}", util::ago_label(entry.modified)),
                        Style::default().fg(faint_color()),
                    ),
                ];
                if entry.is_dir {
                    fourth.push(Span::styled(
                        "   ·   points to a folder",
                        Style::default().fg(blue()),
                    ));
                }
                out.push(Line::from(fourth));
            }
            out.truncate(inner.height as usize);
            out
        }
    };

    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(theme::surface()),
        Rect {
            x: inner.x + 2,
            y: inner.y,
            width: inner.width.saturating_sub(3),
            height: inner.height,
        },
    );
}

fn kind_color(entry: &Entry) -> Color {
    match entry.kind {
        EntryKind::Parent => muted(),
        EntryKind::Dir => blue(),
        EntryKind::File => text_color(),
        EntryKind::Link => amber(),
        EntryKind::Broken | EntryKind::Loop => danger(),
    }
}

fn kind_glyph(entry: &Entry) -> &'static str {
    match entry.kind {
        EntryKind::Parent => ICON_PARENT,
        EntryKind::Dir => ICON_DIR,
        EntryKind::File => ICON_FILE,
        EntryKind::Link => ICON_LINK,
        EntryKind::Broken => ICON_BROKEN,
        EntryKind::Loop => ICON_LOOP,
    }
}

fn kind_word(entry: &Entry) -> &'static str {
    match entry.kind {
        EntryKind::Parent => "parent",
        EntryKind::Dir => "folder",
        EntryKind::File => "file",
        EntryKind::Link => "link",
        EntryKind::Broken => "broken",
        EntryKind::Loop => "loop",
    }
}

// ---------------------------------------------------------------------------
// status + footer
// ---------------------------------------------------------------------------

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let badges = app.status_badges();
    let mut right_spans: Vec<Span> = Vec::new();
    let mut right_width = 0usize;
    for (index, badge) in badges.iter().enumerate() {
        if index > 0 {
            right_spans.push(Span::raw(" "));
            right_width += 1;
        }
        let color = if badge.starts_with("REL") {
            mint()
        } else if badge.starts_with("FORCE") {
            amber()
        } else if badge.starts_with("HIDDEN") {
            violet()
        } else {
            amber()
        };
        right_width += badge.chars().count() + 2;
        right_spans.push(Span::styled(
            format!(" {badge} "),
            Style::default()
                .fg(color)
                .bg(surface_hi())
                .add_modifier(Modifier::BOLD),
        ));
    }

    let left_width = area
        .width
        .saturating_sub(right_width as u16)
        .saturating_sub(1);
    let left_line = match &app.toast {
        Some(toast) => {
            let (icon, color, text_color) = match toast.level {
                ToastLevel::Success => (paint_check(), mint(), text_color()),
                ToastLevel::Error => ("✗", danger(), danger()),
                ToastLevel::Info => ("·", muted(), muted()),
            };
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    icon,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    util::truncate(&toast.text, left_width.saturating_sub(4) as usize),
                    Style::default().fg(text_color),
                ),
            ])
        }
        None => Line::from(vec![
            Span::raw(" "),
            Span::styled("ready", Style::default().fg(muted())),
            Span::styled(
                "   ·   pick a SOURCE, then l links it into DEST",
                Style::default().fg(faint_color()),
            ),
        ]),
    };
    frame.render_widget(
        Paragraph::new(left_line),
        Rect {
            x: area.x,
            y: area.y,
            width: left_width,
            height: 1,
        },
    );

    if !right_spans.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(right_spans)),
            Rect {
                x: area.right().saturating_sub(right_width as u16),
                y: area.y,
                width: right_width as u16,
                height: 1,
            },
        );
    }
}

fn paint_check() -> &'static str {
    "✓"
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let normal = matches!(app.mode, Mode::Normal);
    let reserved = if normal { 9u16 } else { 0 };
    let left_width = area.width.saturating_sub(reserved);
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    let mut used = 1usize;
    for (index, (key, label)) in app.shortcut_hints().into_iter().enumerate() {
        let width = key.chars().count() + label.chars().count() + 3;
        let separator = if index == 0 { 0 } else { 5 };
        if used + width + separator > left_width as usize {
            break;
        }
        if index > 0 {
            spans.push(Span::styled("  ·  ", Style::default().fg(faint_color())));
            used += separator;
        }
        spans.extend(theme::hint(key, label));
        used += width;
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(theme::base()),
        Rect {
            x: area.x,
            width: left_width,
            y: area.y,
            height: 1,
        },
    );

    if normal {
        let quit = Rect {
            x: area.right().saturating_sub(8),
            y: area.y,
            width: 8,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" q ", theme::keycap()),
                Span::styled(" quit", Style::default().fg(muted())),
            ]))
            .style(theme::base()),
            quit,
        );
    }
}

// ---------------------------------------------------------------------------
// dialog chrome
// ---------------------------------------------------------------------------

fn popup_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(4)).max(20);
    let height = height.min(area.height.saturating_sub(2)).max(5);
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

fn popup_chrome(
    frame: &mut Frame,
    area: Rect,
    width: u16,
    height: u16,
    title: &str,
    accent: Color,
) -> Rect {
    let rect = popup_rect(area, width, height);

    let shadow = Rect {
        x: rect.x.saturating_add(1),
        y: rect.y.saturating_add(1),
        width: rect
            .width
            .min(area.right().saturating_sub(rect.x.saturating_add(1))),
        height: rect
            .height
            .min(area.bottom().saturating_sub(rect.y.saturating_add(1))),
    };
    if shadow.width > 0 && shadow.height > 0 {
        frame.render_widget(Clear, shadow);
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Rgb(4, 6, 9))),
            shadow,
        );
    }

    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(surface_hi()))
        .title(Line::from(Span::styled(
            format!(" {title} "),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    inner
}

struct Button {
    key: &'static str,
    label: &'static str,
    action: ModalAction,
    accent: Color,
}

fn draw_buttons(frame: &mut Frame, inner: Rect, y: u16, buttons: &[Button], map: &mut UiMap) {
    let widths: Vec<u16> = buttons
        .iter()
        .map(|button| (button.key.chars().count() + button.label.chars().count() + 5) as u16)
        .collect();
    let total: u16 = widths.iter().sum::<u16>() + 2 * buttons.len().saturating_sub(1) as u16;
    let mut x = inner
        .right()
        .saturating_sub(total)
        .max(inner.x.saturating_add(1));

    for (button, width) in buttons.iter().zip(widths) {
        if x.saturating_add(width) > inner.right() {
            break;
        }
        let rect = Rect {
            x,
            y,
            width,
            height: 1,
        };
        let line = Line::from(vec![
            Span::styled(
                format!(" {} ", button.key),
                Style::default()
                    .fg(bg())
                    .bg(button.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!(" {} ", button.label),
                Style::default().fg(text_color()).bg(border_soft()),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(surface_hi())),
            rect,
        );
        map.modal.push((rect, button.action));
        x = x.saturating_add(width + 2);
    }
}

// ---------------------------------------------------------------------------
// prompt dialog
// ---------------------------------------------------------------------------

fn draw_prompt(frame: &mut Frame, prompt: &PromptState, area: Rect, map: &mut UiMap) {
    let inner = popup_chrome(frame, area, 72, 9, &prompt.title, mint());
    if inner.height == 0 {
        return;
    }
    let content_width = inner.width.saturating_sub(4);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            util::truncate(&prompt.message, content_width as usize),
            Style::default().fg(muted()),
        ))),
        Rect {
            x: inner.x + 2,
            y: inner.y,
            width: content_width,
            height: 1,
        },
    );

    let label = match &prompt.kind {
        PromptKind::Filter(_) => "Filter",
        PromptKind::GoTo(_) => "Path",
        PromptKind::LinkName(_) => "Name",
    };
    let field_rect = Rect {
        x: inner.x + 2,
        y: inner.y.saturating_add(2),
        width: inner.width.saturating_sub(4),
        height: 1,
    };
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(surface_color())),
        field_rect,
    );
    let input_width = field_rect.width.saturating_sub(12).max(4) as usize;
    let (before, at, after) = prompt.input.view(input_width);
    let cursor_style = Style::default()
        .fg(bg())
        .bg(mint())
        .add_modifier(Modifier::BOLD);
    let line = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("{label:<6}  "),
            Style::default().fg(muted()).bg(surface_color()),
        ),
        Span::styled("› ", Style::default().fg(mint()).bg(surface_color())),
        Span::styled(before, Style::default().fg(bright()).bg(surface_color())),
        Span::styled(at.to_string(), cursor_style),
        Span::styled(after, Style::default().fg(bright()).bg(surface_color())),
    ]);
    frame.render_widget(Paragraph::new(line), field_rect);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            util::truncate(&prompt.hint, content_width as usize),
            Style::default().fg(faint_color()),
        ))),
        Rect {
            x: inner.x + 2,
            y: inner.y.saturating_add(4),
            width: content_width,
            height: 1,
        },
    );

    let buttons = [
        Button {
            key: "Enter",
            label: "confirm",
            action: ModalAction::PromptSubmit,
            accent: mint(),
        },
        Button {
            key: "Esc",
            label: "cancel",
            action: ModalAction::PromptCancel,
            accent: muted(),
        },
    ];
    let y = inner.bottom().saturating_sub(1);
    draw_buttons(frame, inner, y, &buttons, map);
}

// ---------------------------------------------------------------------------
// confirm dialog
// ---------------------------------------------------------------------------

fn draw_confirm(frame: &mut Frame, confirm: &ConfirmState, area: Rect, map: &mut UiMap) {
    let height = (confirm.lines.len() as u16 + 6).min(16);
    let inner = popup_chrome(frame, area, 66, height, &confirm.title, danger());
    if inner.height < 4 {
        return;
    }
    let mut lines: Vec<Line> = Vec::new();
    for (index, text) in confirm.lines.iter().enumerate() {
        let style = if index == 0 {
            Style::default().fg(bright()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(muted())
        };
        lines.push(Line::from(Span::styled(
            util::truncate(text, inner.width.saturating_sub(4) as usize),
            style,
        )));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "The link is removed. The file it points to is untouched.",
        Style::default().fg(faint_color()),
    )));

    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(surface_hi())),
        Rect {
            x: inner.x + 2,
            y: inner.y,
            width: inner.width.saturating_sub(4),
            height: inner.height.saturating_sub(2),
        },
    );

    let buttons = [
        Button {
            key: "y",
            label: "remove",
            action: ModalAction::ConfirmYes,
            accent: danger(),
        },
        Button {
            key: "Esc",
            label: "cancel",
            action: ModalAction::ConfirmNo,
            accent: muted(),
        },
    ];
    draw_buttons(
        frame,
        inner,
        inner.bottom().saturating_sub(1),
        &buttons,
        map,
    );
}

// ---------------------------------------------------------------------------
// conflict dialog
// ---------------------------------------------------------------------------

fn draw_conflict(frame: &mut Frame, conflict: &ConflictState, area: Rect, map: &mut UiMap) {
    let shown = conflict.conflicts.len().min(4);
    let extra = conflict.conflicts.len().saturating_sub(shown);
    let height = (shown as u16 + 10 + if extra > 0 { 1 } else { 0 }).min(18);
    let inner = popup_chrome(frame, area, 70, height, "Name conflict", amber());
    if inner.height < 6 {
        return;
    }

    let intro = format!(
        "{} of {} {} already {} in {}",
        conflict.conflicts.len(),
        conflict.names.len(),
        if conflict.conflicts.len() == 1 {
            "name"
        } else {
            "names"
        },
        if conflict.conflicts.len() == 1 {
            "exists"
        } else {
            "exist"
        },
        util::shorten_path(&conflict.dest, 36)
    );
    frame.render_widget(
        Paragraph::new(Span::styled(intro, Style::default().fg(bright()))),
        Rect {
            x: inner.x + 2,
            y: inner.y,
            width: inner.width.saturating_sub(4),
            height: 1,
        },
    );

    for (index, name) in conflict.conflicts.iter().take(shown).enumerate() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  • ", Style::default().fg(amber())),
                Span::styled(
                    util::truncate(name, inner.width.saturating_sub(8) as usize),
                    Style::default().fg(amber()),
                ),
            ])),
            Rect {
                x: inner.x + 2,
                y: inner.y + 1 + index as u16,
                width: inner.width.saturating_sub(4),
                height: 1,
            },
        );
    }
    let mut y = inner.y + 1 + shown as u16;
    if extra > 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("  … and {extra} more"),
                Style::default().fg(faint_color()),
            )),
            Rect {
                x: inner.x + 2,
                y,
                width: inner.width.saturating_sub(4),
                height: 1,
            },
        );
        y += 1;
    }

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Choose what to do with the existing entries:",
            Style::default().fg(muted()),
        )),
        Rect {
            x: inner.x + 2,
            y,
            width: inner.width.saturating_sub(4),
            height: 1,
        },
    );
    y += 1;

    let options = [
        ("Overwrite existing", "o"),
        ("Backup and overwrite", "b"),
        ("Skip existing", "s"),
        ("Cancel", "c"),
    ];
    for (index, (label, key)) in options.iter().enumerate() {
        let selected = index == conflict.choice;
        let rect = Rect {
            x: inner.x + 1,
            y: y + index as u16,
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        let style = if selected {
            Style::default().bg(select_bg())
        } else {
            Style::default().bg(surface_hi())
        };
        let bar_color = if selected { mint() } else { faint_color() };
        let name_style = if selected {
            Style::default().fg(bright()).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(muted())
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    if selected { ICON_CURSOR } else { " " },
                    Style::default().fg(bar_color),
                ),
                Span::raw(" "),
                Span::styled(format!(" {key} "), theme::keycap()),
                Span::styled(format!("  {label}"), name_style),
            ]))
            .style(style),
            rect,
        );
        map.modal.push((rect, ModalAction::ConflictChoice(index)));
    }

    frame.render_widget(
        Paragraph::new(Span::styled(
            "↑↓ choose · Enter apply · Esc cancel",
            Style::default().fg(faint_color()),
        )),
        Rect {
            x: inner.x + 2,
            y: inner.bottom().saturating_sub(1),
            width: inner.width.saturating_sub(4),
            height: 1,
        },
    );
}

// ---------------------------------------------------------------------------
// command palette
// ---------------------------------------------------------------------------

fn draw_palette(frame: &mut Frame, palette: &PaletteState, area: Rect, map: &mut UiMap) {
    let inner = popup_chrome(frame, area, 88, 19, "Command palette", mint());
    if inner.height < 6 {
        return;
    }

    let search = Rect {
        x: inner.x + 2,
        y: inner.y,
        width: inner.width.saturating_sub(4),
        height: 1,
    };
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(surface_color())),
        search,
    );
    let input_width = search.width.saturating_sub(12).max(4) as usize;
    let (before, at, after) = palette.input.view(input_width);
    let cursor_style = Style::default()
        .fg(bg())
        .bg(mint())
        .add_modifier(Modifier::BOLD);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled("⌕  ", Style::default().fg(mint()).bg(surface_color())),
            Span::styled(before, Style::default().fg(bright()).bg(surface_color())),
            Span::styled(at.to_string(), cursor_style),
            Span::styled(after, Style::default().fg(bright()).bg(surface_color())),
        ])),
        search,
    );
    let total_label = format!(
        "{} result{}",
        palette.filtered.len(),
        if palette.filtered.len() == 1 { "" } else { "s" }
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            total_label,
            Style::default().fg(faint_color()),
        ))
        .alignment(Alignment::Right),
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width.saturating_sub(3),
            height: 1,
        },
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(border()),
        )),
        Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        },
    );

    let footer_y = inner.bottom().saturating_sub(1);
    let list_y = inner.y + 2;
    let list_height = footer_y.saturating_sub(list_y) as usize;
    if list_height == 0 {
        return;
    }

    if palette.filtered.is_empty() {
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::default(),
                Line::from(Span::styled("no matches", Style::default().fg(muted()))),
                Line::from(Span::styled(
                    "try a shorter word, or a path fragment",
                    Style::default().fg(faint_color()),
                )),
            ]))
            .alignment(Alignment::Center),
            Rect {
                x: inner.x + 2,
                y: list_y,
                width: inner.width.saturating_sub(4),
                height: list_height as u16,
            },
        );
    } else {
        let offset = if palette.selected >= list_height {
            palette.selected + 1 - list_height
        } else {
            0
        };
        // One cursor cell, an 8-cell group chip and a spacer come first; the
        // rest is split between the title and its description.
        let available = (inner.width as usize).saturating_sub(10);
        let title_width = (available * 45 / 100)
            .clamp(16, 34)
            .min(available.saturating_sub(12).max(1));
        let subtitle_width = available.saturating_sub(title_width + 3).max(1);
        for row in 0..list_height {
            let position = offset + row;
            let Some(index) = palette.filtered.get(position) else {
                break;
            };
            let Some(item) = palette.items.get(*index) else {
                continue;
            };
            let selected = position == palette.selected;
            let group_color = match item.group {
                "SOURCE" => blue(),
                "DEST" => amber(),
                _ => mint(),
            };
            let title_style = if selected {
                Style::default().fg(bright()).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(text_color())
            };
            let bar = if selected { ICON_CURSOR } else { " " };
            let rect = Rect {
                x: inner.x,
                y: list_y + row as u16,
                width: inner.width,
                height: 1,
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        bar,
                        Style::default().fg(if selected { mint() } else { faint_color() }),
                    ),
                    Span::styled(
                        format!(" {:<6} ", item.group),
                        Style::default().fg(group_color).bg(surface_hi()),
                    ),
                    Span::raw(" "),
                    Span::styled(util::truncate(&item.title, title_width), title_style),
                    Span::raw("   "),
                    Span::styled(
                        util::truncate(&item.subtitle, subtitle_width),
                        Style::default().fg(if selected { muted() } else { faint_color() }),
                    ),
                ]))
                .style(Style::default().bg(if selected {
                    select_bg()
                } else {
                    surface_hi()
                })),
                rect,
            );
            map.modal.push((rect, ModalAction::PalettePick(position)));
        }
    }

    let hint = " ↑↓ move · Enter run · Esc close";
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(faint_color()))),
        Rect {
            x: inner.x + 1,
            y: footer_y,
            width: inner.width.saturating_sub(4),
            height: 1,
        },
    );
    let total = format!("{}/{}", palette.filtered.len(), palette.items.len());
    frame.render_widget(
        Paragraph::new(Span::styled(total, Style::default().fg(faint_color())))
            .alignment(Alignment::Right),
        Rect {
            x: inner.x,
            y: footer_y,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );
}

// ---------------------------------------------------------------------------
// help dialog
// ---------------------------------------------------------------------------

fn draw_help(frame: &mut Frame, _app: &App, area: Rect, map: &mut UiMap) {
    let height = 21.min(area.height.saturating_sub(2));
    let inner = popup_chrome(frame, area, 100, height, "Keys & shortcuts", mint());
    if inner.height < 6 {
        return;
    }
    let body_height = inner.height.saturating_sub(2);
    let columns = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .spacing(2)
    .split(Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: body_height,
    });

    let navigate = section(
        "NAVIGATE",
        &[
            ("↑↓ / j k", "move"),
            ("Enter / →", "open"),
            ("← / h", "parent"),
            ("Tab", "pane"),
            ("^L", "jump"),
            ("~", "home"),
            ("g / G", "ends"),
            ("/", "filter"),
            (".", "hidden"),
        ],
    );
    let mut link = section(
        "LINK",
        &[
            ("l", "link → DEST"),
            ("L / n", "new name"),
            ("Space", "mark"),
            ("a", "mark all"),
            ("t", "retarget"),
            ("r", "rel / abs"),
            ("f", "force"),
            ("x", "unlink"),
        ],
    );
    link.extend(section(
        "SESSION",
        &[
            ("1 / 2", "pane"),
            ("R", "refresh"),
            ("u / ^Z", "undo"),
            ("U / ^Y", "redo"),
            ("q", "quit"),
        ],
    ));
    link.pop();
    let mut mouse = section(
        "MOUSE",
        &[
            ("click", "select"),
            ("dbl-click", "open"),
            ("r-click", "mark"),
            ("wheel", "scroll"),
            ("toolbar", "click"),
        ],
    );
    mouse.extend(section(
        "TIPS",
        &[
            ("^P", "palette"),
            ("Esc", "clear"),
            ("SOURCE", "→ links"),
            ("DEST", "← created"),
        ],
    ));
    mouse.pop();

    for (column, lines) in columns.iter().zip([navigate, link, mouse]) {
        frame.render_widget(
            Paragraph::new(Text::from(lines)).style(Style::default().bg(surface_hi())),
            *column,
        );
    }

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Mouse works everywhere.",
            Style::default().fg(faint_color()),
        )),
        Rect {
            x: inner.x + 2,
            y: inner.bottom().saturating_sub(1),
            width: inner.width.saturating_sub(24),
            height: 1,
        },
    );

    let buttons = [Button {
        key: "Esc",
        label: "close",
        action: ModalAction::CloseHelp,
        accent: muted(),
    }];
    draw_buttons(
        frame,
        inner,
        inner.bottom().saturating_sub(1),
        &buttons,
        map,
    );
}

fn section(title: &str, rows: &[(&str, &str)]) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            title.to_string(),
            Style::default().fg(mint()).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "─".repeat(title.chars().count()),
            Style::default().fg(border()),
        )),
    ];
    for (key, label) in rows {
        lines.push(Line::from(vec![
            Span::styled(format!(" {key:<9} "), Style::default().fg(amber())),
            Span::styled(label.to_string(), Style::default().fg(text_color())),
        ]));
    }
    lines.push(Line::default());
    lines
}
