//! Palettes for the TUI. `black` is the default look: a deep slate canvas,
//! mint highlights and amber links. Other themes are inspired by popular
//! editor colour schemes, including a light one for bright terminals.

use std::sync::RwLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

pub struct Theme {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub bg: Color,
    pub surface: Color,
    pub surface_hi: Color,
    pub select_bg: Color,
    pub border: Color,
    pub border_soft: Color,
    pub mint: Color,
    pub mint_dim: Color,
    pub amber: Color,
    pub amber_dim: Color,
    pub blue: Color,
    pub text: Color,
    pub bright: Color,
    pub muted: Color,
    pub faint: Color,
    pub danger: Color,
    pub violet: Color,
    /// Wordmark gradient endpoints, as raw RGB.
    pub wordmark_from: (u8, u8, u8),
    pub wordmark_to: (u8, u8, u8),
}

pub static BLACK: Theme = Theme {
    name: "black",
    aliases: &["default", "slate", "dark"],
    bg: Color::Rgb(11, 14, 20),
    surface: Color::Rgb(16, 21, 29),
    surface_hi: Color::Rgb(23, 30, 41),
    select_bg: Color::Rgb(29, 42, 52),
    border: Color::Rgb(43, 52, 67),
    border_soft: Color::Rgb(31, 38, 50),
    mint: Color::Rgb(122, 224, 193),
    mint_dim: Color::Rgb(73, 141, 124),
    amber: Color::Rgb(235, 181, 113),
    amber_dim: Color::Rgb(150, 117, 77),
    blue: Color::Rgb(128, 179, 231),
    text: Color::Rgb(213, 221, 232),
    bright: Color::Rgb(244, 248, 252),
    muted: Color::Rgb(113, 125, 143),
    faint: Color::Rgb(76, 87, 103),
    danger: Color::Rgb(231, 114, 124),
    violet: Color::Rgb(187, 154, 231),
    wordmark_from: (141, 240, 208),
    wordmark_to: (116, 170, 227),
};

pub static CATPPUCCIN_MOCHA: Theme = Theme {
    name: "catppuccin-mocha",
    aliases: &["catppuccin", "mocha", "catppuccin_mocha"],
    bg: Color::Rgb(30, 30, 46),
    surface: Color::Rgb(24, 24, 37),
    surface_hi: Color::Rgb(49, 50, 68),
    select_bg: Color::Rgb(69, 71, 90),
    border: Color::Rgb(88, 91, 112),
    border_soft: Color::Rgb(49, 50, 68),
    mint: Color::Rgb(166, 227, 161),
    mint_dim: Color::Rgb(64, 160, 43),
    amber: Color::Rgb(250, 179, 135),
    amber_dim: Color::Rgb(254, 100, 11),
    blue: Color::Rgb(137, 180, 250),
    text: Color::Rgb(205, 214, 244),
    bright: Color::Rgb(245, 245, 245),
    muted: Color::Rgb(166, 173, 200),
    faint: Color::Rgb(108, 112, 134),
    danger: Color::Rgb(243, 139, 168),
    violet: Color::Rgb(203, 166, 247),
    wordmark_from: (166, 227, 161),
    wordmark_to: (137, 180, 250),
};

pub static TOKYO_NIGHT: Theme = Theme {
    name: "tokyo-night",
    aliases: &["tokyonight", "tokio-night", "tokio", "tokyo"],
    bg: Color::Rgb(26, 27, 38),
    surface: Color::Rgb(22, 22, 30),
    surface_hi: Color::Rgb(36, 40, 59),
    select_bg: Color::Rgb(41, 46, 66),
    border: Color::Rgb(59, 66, 97),
    border_soft: Color::Rgb(41, 46, 66),
    mint: Color::Rgb(115, 218, 202),
    mint_dim: Color::Rgb(65, 166, 181),
    amber: Color::Rgb(224, 175, 104),
    amber_dim: Color::Rgb(161, 121, 63),
    blue: Color::Rgb(122, 162, 247),
    text: Color::Rgb(192, 202, 245),
    bright: Color::Rgb(255, 255, 255),
    muted: Color::Rgb(122, 134, 173),
    faint: Color::Rgb(86, 95, 137),
    danger: Color::Rgb(247, 118, 142),
    violet: Color::Rgb(187, 154, 247),
    wordmark_from: (115, 218, 202),
    wordmark_to: (122, 162, 247),
};

pub static DRACULA: Theme = Theme {
    name: "dracula",
    aliases: &[],
    bg: Color::Rgb(40, 42, 54),
    surface: Color::Rgb(33, 34, 44),
    surface_hi: Color::Rgb(52, 55, 70),
    select_bg: Color::Rgb(68, 71, 90),
    border: Color::Rgb(98, 114, 164),
    border_soft: Color::Rgb(68, 71, 90),
    mint: Color::Rgb(80, 250, 123),
    mint_dim: Color::Rgb(40, 160, 90),
    amber: Color::Rgb(241, 250, 140),
    amber_dim: Color::Rgb(160, 165, 92),
    blue: Color::Rgb(139, 233, 253),
    text: Color::Rgb(248, 248, 242),
    bright: Color::Rgb(255, 255, 255),
    muted: Color::Rgb(168, 176, 200),
    faint: Color::Rgb(110, 118, 140),
    danger: Color::Rgb(255, 85, 85),
    violet: Color::Rgb(189, 147, 249),
    wordmark_from: (80, 250, 123),
    wordmark_to: (139, 233, 253),
};

pub static GRUVBOX: Theme = Theme {
    name: "gruvbox",
    aliases: &["gruvbox-dark"],
    bg: Color::Rgb(40, 40, 40),
    surface: Color::Rgb(29, 32, 33),
    surface_hi: Color::Rgb(60, 56, 54),
    select_bg: Color::Rgb(80, 73, 69),
    border: Color::Rgb(102, 92, 84),
    border_soft: Color::Rgb(60, 56, 54),
    mint: Color::Rgb(184, 187, 38),
    mint_dim: Color::Rgb(121, 123, 45),
    amber: Color::Rgb(250, 189, 47),
    amber_dim: Color::Rgb(181, 138, 40),
    blue: Color::Rgb(131, 165, 152),
    text: Color::Rgb(235, 219, 178),
    bright: Color::Rgb(251, 241, 199),
    muted: Color::Rgb(168, 153, 132),
    faint: Color::Rgb(124, 111, 100),
    danger: Color::Rgb(251, 73, 52),
    violet: Color::Rgb(211, 134, 155),
    wordmark_from: (250, 189, 47),
    wordmark_to: (131, 165, 152),
};

pub static EVERFOREST: Theme = Theme {
    name: "everforest",
    aliases: &["everforest-dark"],
    bg: Color::Rgb(45, 53, 59),
    surface: Color::Rgb(39, 46, 51),
    surface_hi: Color::Rgb(52, 63, 68),
    select_bg: Color::Rgb(61, 72, 77),
    border: Color::Rgb(71, 82, 88),
    border_soft: Color::Rgb(52, 63, 68),
    mint: Color::Rgb(167, 192, 128),
    mint_dim: Color::Rgb(108, 126, 82),
    amber: Color::Rgb(219, 188, 127),
    amber_dim: Color::Rgb(152, 131, 89),
    blue: Color::Rgb(127, 187, 179),
    text: Color::Rgb(211, 198, 170),
    bright: Color::Rgb(240, 232, 213),
    muted: Color::Rgb(133, 146, 137),
    faint: Color::Rgb(92, 106, 114),
    danger: Color::Rgb(230, 126, 128),
    violet: Color::Rgb(214, 153, 182),
    wordmark_from: (167, 192, 128),
    wordmark_to: (127, 187, 179),
};

pub static GITHUB_LIGHT: Theme = Theme {
    name: "github-light",
    aliases: &["light", "github", "githublight"],
    bg: Color::Rgb(255, 255, 255),
    surface: Color::Rgb(246, 248, 250),
    surface_hi: Color::Rgb(234, 238, 242),
    select_bg: Color::Rgb(221, 244, 255),
    border: Color::Rgb(208, 215, 222),
    border_soft: Color::Rgb(234, 238, 242),
    mint: Color::Rgb(26, 127, 55),
    mint_dim: Color::Rgb(17, 99, 41),
    amber: Color::Rgb(154, 103, 0),
    amber_dim: Color::Rgb(191, 135, 0),
    blue: Color::Rgb(9, 105, 218),
    text: Color::Rgb(31, 35, 40),
    bright: Color::Rgb(0, 0, 0),
    muted: Color::Rgb(89, 99, 110),
    faint: Color::Rgb(129, 139, 152),
    danger: Color::Rgb(207, 34, 46),
    violet: Color::Rgb(130, 80, 223),
    wordmark_from: (26, 127, 55),
    wordmark_to: (9, 105, 218),
};

pub static ALL: [&Theme; 7] = [
    &BLACK,
    &CATPPUCCIN_MOCHA,
    &TOKYO_NIGHT,
    &DRACULA,
    &GRUVBOX,
    &EVERFOREST,
    &GITHUB_LIGHT,
];

static CURRENT: RwLock<&'static Theme> = RwLock::new(&BLACK);

fn normalize(name: &str) -> String {
    name.trim().to_lowercase().replace(['_', ' '], "-")
}

pub fn find(name: &str) -> Option<&'static Theme> {
    let needle = normalize(name);
    ALL.iter().copied().find(|theme| {
        theme.name == needle || theme.aliases.iter().any(|alias| normalize(alias) == needle)
    })
}

pub fn names() -> Vec<&'static str> {
    ALL.iter().map(|theme| theme.name).collect()
}

/// Switch the active palette. Returns the canonical theme name.
pub fn set(name: &str) -> Result<&'static str, String> {
    let theme = find(name).ok_or_else(|| {
        format!(
            "unknown theme `{name}` — try one of: {}",
            names().join(", ")
        )
    })?;
    let mut current = CURRENT.write().unwrap_or_else(|error| error.into_inner());
    *current = theme;
    Ok(theme.name)
}

/// Switch to the next palette in the list and return its name.
pub fn cycle() -> &'static str {
    let mut current = CURRENT.write().unwrap_or_else(|error| error.into_inner());
    let position = ALL
        .iter()
        .position(|theme| std::ptr::eq(*theme, *current))
        .unwrap_or(0);
    let next = ALL[(position + 1) % ALL.len()];
    *current = next;
    next.name
}

pub fn current() -> &'static Theme {
    *CURRENT.read().unwrap_or_else(|error| error.into_inner())
}

pub fn name() -> &'static str {
    current().name
}

pub fn bg() -> Color {
    current().bg
}
pub fn surface_color() -> Color {
    current().surface
}
pub fn surface_hi() -> Color {
    current().surface_hi
}
pub fn select_bg() -> Color {
    current().select_bg
}
pub fn border() -> Color {
    current().border
}
pub fn border_soft() -> Color {
    current().border_soft
}
pub fn mint() -> Color {
    current().mint
}
pub fn mint_dim() -> Color {
    current().mint_dim
}
pub fn amber() -> Color {
    current().amber
}
pub fn amber_dim() -> Color {
    current().amber_dim
}
pub fn blue() -> Color {
    current().blue
}
pub fn text_color() -> Color {
    current().text
}
pub fn bright() -> Color {
    current().bright
}
pub fn muted() -> Color {
    current().muted
}
pub fn faint_color() -> Color {
    current().faint
}
pub fn danger() -> Color {
    current().danger
}
pub fn violet() -> Color {
    current().violet
}

pub const ICON_DIR: &str = "▸";
pub const ICON_FILE: &str = "·";
pub const ICON_LINK: &str = "↗";
pub const ICON_BROKEN: &str = "✗";
pub const ICON_LOOP: &str = "↻";
pub const ICON_PARENT: &str = "↑";
pub const ICON_MARK: &str = "●";
pub const ICON_CURSOR: &str = "▌";

pub fn base() -> Style {
    Style::default().fg(text_color()).bg(bg())
}

pub fn surface() -> Style {
    Style::default().fg(text_color()).bg(surface_color())
}

pub fn faint() -> Style {
    Style::default().fg(faint_color())
}

pub fn keycap() -> Style {
    Style::default()
        .fg(mint())
        .bg(surface_hi())
        .add_modifier(Modifier::BOLD)
}

/// Keyboard hint: a highlighted key and a quiet label.
pub fn hint<'a>(key: impl Into<String>, label: impl Into<String>) -> Vec<Span<'a>> {
    vec![
        Span::styled(format!(" {} ", key.into()), keycap()),
        Span::styled(format!(" {}", label.into()), Style::default().fg(muted())),
    ]
}

/// A subtle left-to-right gradient used for the wordmark.
pub fn wordmark(text: &str) -> Vec<Span<'static>> {
    let theme = current();
    let chars: Vec<char> = text.chars().collect();
    let last = chars.len().saturating_sub(1).max(1);
    chars
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let t = i as f32 / last as f32;
            let color = lerp(theme.wordmark_from, theme.wordmark_to, t);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// These tests share the process-wide palette, so they run one at a time.
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn every_theme_has_a_unique_name() {
        let mut seen = std::collections::HashSet::new();
        for theme in ALL {
            assert!(seen.insert(theme.name), "duplicate theme {}", theme.name);
            assert!(find(theme.name).is_some());
        }
        assert_eq!(ALL.len(), 7);
    }

    #[test]
    fn aliases_and_loose_spelling_resolve() {
        assert_eq!(find("Catppuccin Mocha").unwrap().name, "catppuccin-mocha");
        assert_eq!(find("tokio_night").unwrap().name, "tokyo-night");
        assert_eq!(find("TOKYOnight").unwrap().name, "tokyo-night");
        assert_eq!(find("light").unwrap().name, "github-light");
        assert!(find("nope").is_none());
    }

    #[test]
    fn defaults_to_black() {
        let _guard = LOCK.lock().unwrap_or_else(|error| error.into_inner());
        set("black").unwrap();
        assert_eq!(name(), "black");
    }

    #[test]
    fn set_reports_unknown_themes() {
        let _guard = LOCK.lock().unwrap_or_else(|error| error.into_inner());
        set("black").unwrap();
        let error = set("solarized").unwrap_err();
        assert!(error.contains("catppuccin-mocha"));
        assert_eq!(name(), "black");
    }

    #[test]
    fn cycle_walks_every_theme_and_returns_home() {
        let _guard = LOCK.lock().unwrap_or_else(|error| error.into_inner());
        set("black").unwrap();
        let started = name();
        for _ in 0..ALL.len() {
            cycle();
        }
        assert_eq!(name(), started);
        set("black").unwrap();
    }

    #[test]
    fn light_theme_has_light_background_and_dark_text() {
        let theme = find("github-light").unwrap();
        assert!(matches!(theme.bg, Color::Rgb(255, 255, 255)));
        let Color::Rgb(r, g, b) = theme.text else {
            panic!("expected rgb");
        };
        assert!(u16::from(r) + u16::from(g) + u16::from(b) < 200);
    }
}
