//! Terminal lifecycle and the event loop for the interactive UI.

mod app;
mod entry;
mod input;
mod palette;
mod theme;
mod ui;
mod undo;

use std::io::{IsTerminal, Stdout, stdout};
use std::panic;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, bail};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::cursor::{Hide, Show};
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use self::app::{App, ToastLevel};

pub fn run(dir: Option<PathBuf>, dest: Option<PathBuf>) -> Result<()> {
    if !stdout().is_terminal() {
        bail!("the TUI needs a real terminal — use the CLI commands when piping output");
    }

    let loaded = crate::config::load();
    let cfg = loaded.config.clone();
    let cwd = std::env::current_dir()?;
    let mut warning = loaded.warning.clone();
    let mut created: Option<PathBuf> = None;
    if loaded.path.is_none() {
        match crate::config::ensure_default() {
            Ok(path) => created = path,
            Err(err) => {
                let message = format!("cannot create config: {err}");
                warning = Some(match warning {
                    Some(previous) => format!("{previous} · {message}"),
                    None => message,
                });
            }
        }
    }

    if let Err(err) = theme::set(&cfg.theme) {
        warning = Some(match warning {
            Some(previous) => format!("{previous} · {err}"),
            None => err,
        });
    }

    let source = match dir {
        Some(path) => resolve_directory(path)?,
        None => match &cfg.source {
            Some(path) => {
                resolve_configured(path, "source", &mut warning).unwrap_or_else(|| cwd.clone())
            }
            None => cwd.clone(),
        },
    };
    let dest = match dest {
        Some(path) => resolve_directory(path)?,
        None => match &cfg.dest {
            Some(path) => {
                resolve_configured(path, "dest", &mut warning).unwrap_or_else(|| cwd.clone())
            }
            None => cwd.clone(),
        },
    };

    let mut app = App::new(source, dest, &cfg);
    if let Some(warning) = warning {
        app.set_toast(ToastLevel::Error, warning);
    }
    if let Some(path) = created {
        app.set_toast(
            ToastLevel::Info,
            format!("created {}", crate::util::tildify(&path)),
        );
    }

    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        original_hook(info);
    }));

    enable_raw_mode()?;
    let _session = Session;

    let mut out = stdout();
    execute!(
        out,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste,
        Hide
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;
    terminal.clear()?;

    let result = event_loop(&mut terminal, &mut app);

    drop(terminal);
    restore_terminal();
    result
}

fn resolve_directory(path: PathBuf) -> Result<PathBuf> {
    let absolute = crate::fsops::absolutize(&path)?;
    if !absolute.is_dir() {
        bail!("not a directory: {}", path.display());
    }
    Ok(absolute)
}

fn resolve_configured(
    path: &std::path::Path,
    label: &str,
    warning: &mut Option<String>,
) -> Option<PathBuf> {
    match resolve_directory(path.to_path_buf()) {
        Ok(path) => Some(path),
        Err(err) => {
            *warning = Some(format!("config {label} folder ignored: {err}"));
            None
        }
    }
}

struct Session;

impl Drop for Session {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let mut out = stdout();
    let _ = execute!(
        out,
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen,
        Show
    );
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if app.quit {
            break;
        }

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    app.handle_key(key)?;
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse)?;
                }
                Event::Paste(text) => app.handle_paste(text),
                _ => {}
            }
        }

        app.tick();
    }
    Ok(())
}
