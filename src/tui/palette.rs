//! The Ctrl+P fuzzy command palette: items, scoring and filtering.
//!
//! Scoring is fzf-flavoured: a match inside the title beats the same match in
//! a subtitle, word starts beat the middle of words, contiguous runs beat
//! scattered letters and short titles win ties. Queries with a capital letter
//! are case sensitive, so `Link` will not surface `unlink`.

use std::path::PathBuf;

use super::input::TextInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteCommand {
    Link,
    LinkAs,
    Retarget,
    Remove,
    Undo,
    Redo,
    CycleTheme,
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
    title_key: String,
    subtitle_key: String,
    group_key: String,
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
        Self {
            title_key: title.to_lowercase(),
            subtitle_key: subtitle.to_lowercase(),
            group_key: group.to_lowercase(),
            title,
            subtitle,
            group,
            action,
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
            if let Some(score) = score_item(&query, item) {
                scored.push((score, index));
            }
        }
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| {
                    self.items[a.1]
                        .title
                        .chars()
                        .count()
                        .cmp(&self.items[b.1].title.chars().count())
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

const TITLE_BONUS: i64 = 320;
const SUBTITLE_BONUS: i64 = 60;
const GROUP_BONUS: i64 = 20;

const BOUNDARY: [char; 11] = [' ', '/', '\\', '_', '-', '.', ':', '>', '→', '·', ','];

fn is_boundary(c: char) -> bool {
    BOUNDARY.contains(&c)
}

/// Score one item against the whole query. Every whitespace-separated token
/// has to match at least one field; the best field wins for that token.
pub fn score_item(query: &str, item: &PaletteItem) -> Option<i64> {
    let tokens: Vec<&str> = query.split_whitespace().collect();
    if tokens.is_empty() {
        return Some(0);
    }
    let sensitive = query.chars().any(char::is_uppercase);
    let mut total = 0i64;
    for token in tokens {
        let fields = if sensitive {
            [
                (item.title.as_str(), TITLE_BONUS),
                (item.subtitle.as_str(), SUBTITLE_BONUS),
                (item.group, GROUP_BONUS),
            ]
        } else {
            [
                (item.title_key.as_str(), TITLE_BONUS),
                (item.subtitle_key.as_str(), SUBTITLE_BONUS),
                (item.group_key.as_str(), GROUP_BONUS),
            ]
        };
        let mut best: Option<i64> = None;
        for (haystack, bonus) in fields {
            if let Some(score) = field_score(token, haystack) {
                let weighted = score + bonus;
                best = Some(best.map_or(weighted, |current: i64| current.max(weighted)));
            }
        }
        total += best?;
    }
    Some(total)
}

/// Convenience wrapper used by the tests and for single-string matching.
/// Smart case applies: an uppercase character makes the match case sensitive.
#[cfg(test)]
pub fn fuzzy_score(query: &str, haystack: &str) -> Option<i64> {
    let sensitive = query.chars().any(char::is_uppercase);
    if sensitive {
        return score_tokens(query, haystack);
    }
    score_tokens(&query.to_lowercase(), &haystack.to_lowercase())
}

#[cfg(test)]
fn score_tokens(query: &str, haystack: &str) -> Option<i64> {
    let mut total = 0i64;
    for token in query.split_whitespace() {
        total += field_score(token, haystack)?;
    }
    Some(total)
}

fn field_score(token: &str, haystack: &str) -> Option<i64> {
    if token.is_empty() {
        return Some(0);
    }
    if haystack == token {
        return Some(1600);
    }
    let hay_len = haystack.chars().count() as i64;
    let token_len = token.chars().count() as i64;

    if let Some(index) = haystack.find(token) {
        let index = haystack[..index].chars().count() as i64;
        let word_start = index == 0
            || haystack
                .chars()
                .nth(index as usize - 1)
                .is_some_and(is_boundary);
        let mut score = 620 + token_len * 9;
        if index == 0 {
            score += 420;
        } else if word_start {
            score += 190;
        }
        score -= index.min(140);
        // Prefer fields where the match covers more of the text.
        score -= (hay_len - token_len).min(160) / 3;
        return Some(score);
    }

    subsequence_score(token, haystack)
}

fn subsequence_score(token: &str, haystack: &str) -> Option<i64> {
    let query: Vec<char> = token.chars().collect();
    let chars: Vec<char> = haystack.chars().collect();
    let mut matched = 0usize;
    let mut score = 120 + query.len() as i64 * 12;
    let mut previous: Option<usize> = None;
    let mut boundaries = 0usize;

    for (index, c) in chars.iter().enumerate() {
        if matched >= query.len() || *c != query[matched] {
            continue;
        }
        let boundary = index == 0 || is_boundary(chars[index - 1]);
        if boundary {
            score += 34;
            boundaries += 1;
        }
        if let Some(prev) = previous {
            let gap = index.saturating_sub(prev + 1);
            if gap == 0 {
                score += 24;
            } else {
                score -= (gap as i64).min(30);
            }
        }
        previous = Some(index);
        matched += 1;
    }

    if matched == query.len() {
        // Every letter starting a word (e.g. `tgrel` → toggle relative) is a
        // strong signal, as is a match on the first characters.
        if boundaries == query.len() {
            score += 140;
        }
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
        let boundary = fuzzy_score("link", "toggle sym link").unwrap();
        assert!(prefix > later);
        assert!(prefix > boundary);
    }

    #[test]
    fn smart_case_requires_exact_casing_when_capitalised() {
        assert!(fuzzy_score("Link", "link selected").is_none());
        assert!(fuzzy_score("Link", "Link selected").is_some());
        assert!(fuzzy_score("link", "Link selected").is_some());
        assert!(fuzzy_score("README", "README.md").is_some());
        assert!(fuzzy_score("README", "readme.md").is_none());
    }

    #[test]
    fn contiguous_runs_beat_scattered_letters() {
        let run = fuzzy_score("relat", "relative links").unwrap();
        let scattered = fuzzy_score("relat", "remove link at temp").unwrap();
        assert!(run > scattered);
    }

    #[test]
    fn subsequence_and_initials_match() {
        assert!(fuzzy_score("tgrel", "toggle relative links").is_some());
        assert!(fuzzy_score("gf", "go home").is_none());
        assert!(fuzzy_score("gh", "go home").is_some());
        assert_eq!(fuzzy_score("zzz", "toggle hidden files"), None);
    }

    #[test]
    fn title_matches_outrank_subtitle_matches() {
        let state = PaletteState::new(vec![
            item("Refresh panes", "R", "ACTION", PaletteCommand::Refresh),
            item(
                "Jump to path",
                "refresh the view",
                "ACTION",
                PaletteCommand::GoTo,
            ),
            item("Quit", "q", "ACTION", PaletteCommand::Quit),
        ]);
        let mut state = state;
        state.input.insert_str("refresh");
        state.refilter();
        assert_eq!(state.filtered.len(), 2);
        assert_eq!(
            state.selected_action(),
            Some(PaletteAction::Command(PaletteCommand::Refresh))
        );
    }

    #[test]
    fn multiple_tokens_must_all_match() {
        assert!(fuzzy_score("notes app", "/home/me/notes.md /tmp/app").is_some());
        assert!(fuzzy_score("notes nope", "/home/me/notes.md /tmp/app").is_none());
        let themed = item(
            "Next theme",
            "palette · currently black",
            "LOOK",
            PaletteCommand::CycleTheme,
        );
        assert!(score_item("theme black", &themed).is_some());
        assert!(score_item("theme nope", &themed).is_none());
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

    #[test]
    fn equal_scores_prefer_shorter_titles() {
        let items = vec![
            item(
                "Toggle relative links",
                "r",
                "ACTION",
                PaletteCommand::ToggleRelative,
            ),
            item("Relative", "r", "ACTION", PaletteCommand::ToggleRelative),
        ];
        let mut state = PaletteState::new(items);
        state.input.insert_str("relative");
        state.refilter();
        assert_eq!(
            state.selected_item().map(|item| item.title.as_str()),
            Some("Relative")
        );
    }
}
