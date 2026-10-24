//! Small formatting helpers shared by the CLI and the TUI.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::fsops;

/// Human readable byte size with binary units.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Replace the home directory prefix with `~`.
pub fn tildify(path: &Path) -> String {
    let display = path.display().to_string();
    if let Some(home) = fsops::home_dir() {
        let home = home.display().to_string();
        if display == home {
            return "~".to_string();
        }
        if let Some(rest) = display.strip_prefix(&home)
            && rest.starts_with('/')
        {
            return format!("~{rest}");
        }
    }
    display
}

/// Shorten a path for a title bar, keeping both ends readable.
pub fn shorten_path(path: &Path, max_chars: usize) -> String {
    shorten_middle(&tildify(path), max_chars)
}

/// Truncate from the middle with an ellipsis.
pub fn shorten_middle(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars || max_chars < 5 {
        return text.to_string();
    }
    let keep = max_chars - 1;
    let left = keep.div_ceil(2);
    let right = keep - left;
    let mut out: String = chars[..left].iter().collect();
    out.push('…');
    out.extend(chars[chars.len() - right..].iter());
    out
}

/// Truncate on the right with an ellipsis.
pub fn truncate(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars || max_chars == 0 {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Friendly modified label: `just now`, `4m ago`, `2h ago`.
pub fn ago_label(secs: Option<u64>) -> String {
    let label = relative_time(secs);
    if label == "now" {
        "just now".to_string()
    } else if label == "—" {
        label
    } else {
        format!("{label} ago")
    }
}

/// Compact "time ago" label, e.g. `4m`, `2h`, `3d`.
pub fn relative_time(secs: Option<u64>) -> String {
    let Some(secs) = secs else {
        return "—".to_string();
    };
    let now = now_secs();
    if secs >= now {
        return "now".to_string();
    }
    let delta = now - secs;
    match delta {
        0..=59 => "now".to_string(),
        60..=3599 => format!("{}m", delta / 60),
        3600..=86_399 => format!("{}h", delta / 3600),
        86_400..=604_799 => format!("{}d", delta / 86_400),
        604_800..=2_591_999 => format!("{}w", delta / 604_800),
        2_592_000..=31_535_999 => format!("{}mo", delta / 2_592_000),
        _ => format!("{}y", delta / 31_536_000),
    }
}

/// UTC timestamp as `YYYY-MM-DD HH:MM`.
pub fn format_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let (hour, minute) = (rem / 3600, (rem % 3600) / 60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(1024 * 1024 * 200), "200 MB");
    }

    #[test]
    fn timestamps_are_utc() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00");
        assert_eq!(format_timestamp(1_790_000_000), "2026-09-21 14:13");
    }

    #[test]
    fn middle_shortening_keeps_ends() {
        assert_eq!(shorten_middle("abcdefghij", 6), "abc…ij");
        assert_eq!(shorten_middle("short", 10), "short");
    }

    #[test]
    fn relative_labels() {
        assert_eq!(relative_time(Some(now_secs())), "now");
        assert_eq!(relative_time(None), "—");
    }
}
