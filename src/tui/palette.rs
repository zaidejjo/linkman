//! The Ctrl+P fuzzy command palette: items, matching and filtering.

use std::path::PathBuf;

use super::input::TextInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteCommand {
    Link,
    LinkAs,
    Retarget,
    Remove,
    Undo,
    ToggleRelative,
    ToggleForce,
    ToggleHidden,
    Refresh,
    Filter,
    GoTo,
    Home,
    SwitchPane,
    OpenSelected,
    Mark,
    MarkAll,
    ClearMarks,
    Help,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    Command(PaletteCommand),
    Reveal {
        pane: usize,
        path: PathBuf,
        is_dir: bool,
    },
}

#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub title: String,
    pub subtitle: String,
    pub group: &'static str,
    pub action: PaletteAction,
    search: String,
}

impl PaletteItem {
    pub fn new(
        title: impl Into<String>,
        subtitle: impl Into<String>,
        group: &'static str,
        action: PaletteAction,
    ) -> Self {
        let title = title.into();
        let subtitle = subtitle.into();
        let search = format!("{title} {subtitle} {group}").to_lowercase();
        Self {
            title,
            subtitle,
            group,
            action,
            search,
        }
    }
}

pub struct PaletteState {
    pub input: TextInput,
    pub items: Vec<PaletteItem>,
    pub filtered: Vec<usize>,
    pub selected: usize,
}

impl PaletteState {
    pub fn new(items: Vec<PaletteItem>) -> Self {
        let filtered = (0..items.len()).collect();
        Self {
            input: TextInput::default(),
            items,
            filtered,
            selected: 0,
        }
    }

    pub fn refilter(&mut self) {
        let query = self.input.value().trim().to_string();
        if query.is_empty() {
            self.filtered = (0..self.items.len()).collect();
            self.selected = 0;
            return;
        }
        let mut scored: Vec<(i64, usize)> = Vec::new();
        for (index, item) in self.items.iter().enumerate() {
            if let Some(score) = fuzzy_score(&query, &item.search) {
                scored.push((score, index));
            }
        }
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| {
                    self.items[a.1]
                        .title
                        .len()
                        .cmp(&self.items[b.1].title.len())
                })
                .then_with(|| a.1.cmp(&b.1))
        });
        self.filtered = scored.into_iter().map(|(_, index)| index).collect();
        self.selected = 0;
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            self.selected = 0;
            return;
        }
        let last = self.filtered.len().saturating_sub(1) as isize;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        self.selected = self.filtered.len().saturating_sub(1);
    }

    pub fn selected_item(&self) -> Option<&PaletteItem> {
        self.filtered
            .get(self.selected)
            .and_then(|index| self.items.get(*index))
    }

    pub fn selected_action(&self) -> Option<PaletteAction> {
        self.selected_item().map(|item| item.action.clone())
    }
}

const BOUNDARY: [char; 10] = [' ', '/', '\\', '_', '-', '.', ':', '>', '→', '·'];

fn is_boundary(c: char) -> bool {
    BOUNDARY.contains(&c)
}

/// Cheap fzf-flavoured scoring: exact substrings beat scattered subsequences,
/// and starts of words get a healthy bonus. Every whitespace-separated token
/// must match.
pub fn fuzzy_score(query: &str, haystack: &str) -> Option<i64> {
    let tokens: Vec<String> = query
        .split_whitespace()
        .map(|token| token.to_lowercase())
        .collect();
    if tokens.is_empty() {
        return Some(0);
    }
    let haystack = haystack.to_lowercase();
    let mut total = 0i64;
    for token in tokens {
        total += match_token(&token, &haystack)?;
    }
    Some(total)
}

fn match_token(token: &str, haystack: &str) -> Option<i64> {
    if let Some(index) = haystack.find(token) {
        let boundary = index == 0
            || haystack[..index]
                .chars()
                .next_back()
                .is_some_and(is_boundary);
        let mut score = 900 - (index as i64).min(120) + token.chars().count() as i64 * 4;
        if index == 0 {
            score += 320;
        } else if boundary {
            score += 150;
        }
        return Some(score);
    }

    let query: Vec<char> = token.chars().collect();
    if query.is_empty() {
        return Some(0);
    }
    let chars: Vec<char> = haystack.chars().collect();
    let mut matched = 0usize;
    let mut score = 0i64;
    let mut previous: Option<usize> = None;
    for (index, c) in chars.iter().enumerate() {
        if matched < query.len() && *c == query[matched] {
            score += 10;
            if index == 0 || is_boundary(chars[index - 1]) {
                score += 22;
            }
            if previous == Some(index.saturating_sub(1)) {
                score += 16;
            }
            previous = Some(index);
            matched += 1;
        }
    }
    if matched == query.len() {
        Some(score)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, subtitle: &str, group: &'static str, kind: PaletteCommand) -> PaletteItem {
        PaletteItem::new(title, subtitle, group, PaletteAction::Command(kind))
    }

    #[test]
    fn prefix_and_word_boundaries_win() {
        let prefix = fuzzy_score("link", "link selected → dest").unwrap();
        let later = fuzzy_score("link", "unlink symlinks").unwrap();
        assert!(prefix > later);
        assert!(fuzzy_score("dest", "link selected → dest").is_some());
    }

    #[test]
    fn subsequence_matching_works() {
        assert!(fuzzy_score("tgrel", "toggle relative links").is_some());
        assert!(fuzzy_score("hide", "toggle hidden files").is_some());
        assert_eq!(fuzzy_score("zzz", "toggle hidden files"), None);
    }

    #[test]
    fn multiple_tokens_must_all_match() {
        assert!(fuzzy_score("notes app", "/home/me/notes.md /tmp/app").is_some());
        assert!(fuzzy_score("notes nope", "/home/me/notes.md /tmp/app").is_none());
    }

    #[test]
    fn filtering_sorts_and_limits() {
        let items = vec![
            item("Link selected", "l", "ACTION", PaletteCommand::Link),
            item(
                "Toggle relative",
                "r",
                "ACTION",
                PaletteCommand::ToggleRelative,
            ),
            item("Quit", "q", "ACTION", PaletteCommand::Quit),
        ];
        let mut state = PaletteState::new(items);
        state.input.insert_str("link");
        state.refilter();
        assert_eq!(state.filtered.len(), 1);
        assert_eq!(
            state.selected_action(),
            Some(PaletteAction::Command(PaletteCommand::Link))
        );

        state.input.clear();
        state.refilter();
        assert_eq!(state.filtered.len(), 3);
        state.move_selection(10);
        assert_eq!(state.selected, 2);
    }
}
