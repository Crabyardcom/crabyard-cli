pub mod banner;
pub mod ferris;
pub mod generation;
pub mod theme;
pub mod widgets;
pub mod wizard;

use std::io::{self, stdout};
use std::panic;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;

pub type Terminal = ratatui::Terminal<CrosstermBackend<io::Stdout>>;

pub fn enter_tui() -> io::Result<Terminal> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;

    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = leave_tui_raw();
        prev_hook(info);
    }));

    ratatui::Terminal::new(CrosstermBackend::new(stdout()))
}

pub fn leave_tui() -> io::Result<()> {
    leave_tui_raw()?;
    let _ = panic::take_hook();
    Ok(())
}

fn leave_tui_raw() -> io::Result<()> {
    stdout().execute(DisableMouseCapture)?;
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

pub fn poll_quit(timeout: std::time::Duration) -> bool {
    if event::poll(timeout).unwrap_or(false) {
        if let Ok(Event::Key(k)) = event::read() {
            if k.kind != KeyEventKind::Release
                && matches!(k.code, KeyCode::Char('q') | KeyCode::Esc)
            {
                return true;
            }
        }
    }
    false
}
