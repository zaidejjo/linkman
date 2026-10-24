//! The look of linkman: a deep slate canvas, mint highlights and amber links.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

pub const BG: Color = Color::Rgb(11, 14, 20);
pub const SURFACE: Color = Color::Rgb(16, 21, 29);
pub const SURFACE_HI: Color = Color::Rgb(23, 30, 41);
pub const SELECT_BG: Color = Color::Rgb(29, 42, 52);
pub const BORDER: Color = Color::Rgb(43, 52, 67);
pub const BORDER_SOFT: Color = Color::Rgb(31, 38, 50);
pub const MINT: Color = Color::Rgb(122, 224, 193);
pub const MINT_DIM: Color = Color::Rgb(73, 141, 124);
pub const AMBER: Color = Color::Rgb(235, 181, 113);
pub const AMBER_DIM: Color = Color::Rgb(150, 117, 77);
pub const BLUE: Color = Color::Rgb(128, 179, 231);
pub const TEXT: Color = Color::Rgb(213, 221, 232);
pub const BRIGHT: Color = Color::Rgb(244, 248, 252);
pub const MUTED: Color = Color::Rgb(113, 125, 143);
pub const FAINT: Color = Color::Rgb(76, 87, 103);
pub const DANGER: Color = Color::Rgb(231, 114, 124);
pub const VIOLET: Color = Color::Rgb(187, 154, 231);

pub const ICON_DIR: &str = "▸";
pub const ICON_FILE: &str = "·";
pub const ICON_LINK: &str = "↗";
pub const ICON_BROKEN: &str = "✗";
pub const ICON_LOOP: &str = "↻";
pub const ICON_PARENT: &str = "↑";
pub const ICON_MARK: &str = "●";
pub const ICON_CURSOR: &str = "▌";

pub fn base() -> Style {
    Style::default().fg(TEXT).bg(BG)
}

pub fn surface() -> Style {
    Style::default().fg(TEXT).bg(SURFACE)
}

pub fn faint() -> Style {
    Style::default().fg(FAINT)
}

pub fn keycap() -> Style {
    Style::default()
        .fg(MINT)
        .bg(SURFACE_HI)
        .add_modifier(Modifier::BOLD)
}

/// Keyboard hint: a highlighted key and a quiet label.
pub fn hint<'a>(key: impl Into<String>, label: impl Into<String>) -> Vec<Span<'a>> {
    vec![
        Span::styled(format!(" {} ", key.into()), keycap()),
        Span::styled(format!(" {}", label.into()), Style::default().fg(MUTED)),
    ]
}

/// A subtle left-to-right gradient used for the wordmark.
pub fn wordmark(text: &str) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    let last = chars.len().saturating_sub(1).max(1);
    chars
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let t = i as f32 / last as f32;
            let color = lerp((141, 240, 208), (116, 170, 227), t);
            Span::styled(
                c.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )
        })
        .collect()
}

fn lerp(from: (u8, u8, u8), to: (u8, u8, u8), t: f32) -> Color {
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color::Rgb(mix(from.0, to.0), mix(from.1, to.1), mix(from.2, to.2))
}
