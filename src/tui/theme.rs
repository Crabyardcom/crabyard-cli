use ratatui::style::{Color, Modifier, Style};

pub const ACCENT: Color = Color::Rgb(6, 182, 212);
pub const GREEN: Color = Color::Rgb(34, 197, 94);
pub const RED: Color = Color::Rgb(239, 68, 68);
pub const DIM: Color = Color::Rgb(113, 113, 122);
pub const TEXT: Color = Color::Rgb(228, 228, 231);
pub const MUTED: Color = Color::Rgb(161, 161, 170);
pub const BG: Color = Color::Rgb(9, 9, 11);
pub const BORDER_COLOR: Color = Color::Rgb(39, 39, 42);

pub fn title() -> Style {
    Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
}

pub fn subtitle() -> Style {
    Style::default().fg(MUTED)
}

pub fn accent() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn success() -> Style {
    Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
}

pub fn running() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn pending() -> Style {
    Style::default().fg(DIM)
}

pub fn error() -> Style {
    Style::default().fg(RED).add_modifier(Modifier::BOLD)
}

pub fn progress_filled() -> Style {
    Style::default().fg(ACCENT)
}

pub fn text() -> Style {
    Style::default().fg(TEXT)
}

pub fn border() -> Style {
    Style::default().fg(BORDER_COLOR)
}

pub fn border_highlight() -> Style {
    Style::default().fg(ACCENT)
}

pub fn selected() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn hint() -> Style {
    Style::default().fg(DIM)
}
