use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use super::theme;
use super::widgets::multi_select::{MultiSelectItem, MultiSelectState};
use super::widgets::select::{SelectItem, SelectState};
use super::widgets::text_input::{TextField, TextInputState};

use crate::tokens;

pub struct WizardResult {
    pub pm: String,
    pub auth: String,
    pub ui: String,
    pub extras: Vec<String>,
    pub app_name: String,
    pub slug: String,
    pub bundle_id: String,
    pub version: String,
    pub author: String,
    pub description: String,
}

#[derive(PartialEq)]
enum Screen {
    PackageManager,
    Modules,
    Extras,
    Metadata,
    Summary,
}

impl Screen {
    fn index(&self) -> usize {
        match self {
            Self::PackageManager => 0,
            Self::Modules => 1,
            Self::Extras => 2,
            Self::Metadata => 3,
            Self::Summary => 4,
        }
    }

    fn total() -> usize {
        5
    }
}

enum ModulePhase {
    Auth,
    Ui,
}

struct WizardState {
    screen: Screen,
    pm_select: SelectState,
    auth_select: SelectState,
    ui_select: SelectState,
    module_phase: ModulePhase,
    extras_select: MultiSelectState,
    metadata: TextInputState,
    tick: usize,
    content_area: Rect,
}

pub fn run_wizard(
    pre_pm: Option<&str>,
    pre_auth: Option<&str>,
    pre_ui: Option<&str>,
    pre_extras: &[String],
    pre_name: Option<&str>,
    pre_slug: Option<&str>,
    pre_bundle_id: Option<&str>,
    pre_version: Option<&str>,
    pre_author: Option<&str>,
    pre_description: Option<&str>,
) -> anyhow::Result<Option<WizardResult>> {
    let pm_items = vec![
        SelectItem { label: "bun".into(), description: "Fast all-in-one JS runtime".into() },
        SelectItem { label: "pnpm".into(), description: "Fast, disk space efficient".into() },
        SelectItem { label: "yarn".into(), description: "Classic package manager".into() },
        SelectItem { label: "npm".into(), description: "Node.js default".into() },
    ];
    let mut pm_select = SelectState::new(pm_items);
    if let Some(pm) = pre_pm {
        let idx = ["bun", "pnpm", "yarn", "npm"]
            .iter()
            .position(|&p| p == pm)
            .unwrap_or(0);
        pm_select.cursor = idx;
    }

    let auth_items = vec![
        SelectItem { label: "github".into(), description: "GitHub Device Flow OAuth".into() },
        SelectItem { label: "google".into(), description: "Google PKCE Loopback OAuth".into() },
        SelectItem { label: "none".into(), description: "No authentication".into() },
    ];
    let mut auth_select = SelectState::new(auth_items);
    if let Some(auth) = pre_auth {
        let idx = ["github", "google", "none"]
            .iter()
            .position(|&a| a == auth)
            .unwrap_or(0);
        auth_select.cursor = idx;
    }

    let ui_items = vec![
        SelectItem { label: "shadcn".into(), description: "Radix + Tailwind components".into() },
        SelectItem { label: "daisyui".into(), description: "Tailwind component library".into() },
        SelectItem { label: "tesign".into(), description: "Custom design system".into() },
        SelectItem { label: "minimal".into(), description: "Tailwind only, no components".into() },
    ];
    let mut ui_select = SelectState::new(ui_items);
    if let Some(ui) = pre_ui {
        let idx = ["shadcn", "daisyui", "tesign", "minimal"]
            .iter()
            .position(|&u| u == ui)
            .unwrap_or(0);
        ui_select.cursor = idx;
    }

    let extras_items = vec![
        MultiSelectItem { name: "notifications".into(), label: "Notifications — system notifications".into(), checked: false },
        MultiSelectItem { name: "clipboard".into(), label: "Clipboard — read/write system clipboard".into(), checked: false },
        MultiSelectItem { name: "global-shortcut".into(), label: "Global Shortcuts — system-wide hotkeys".into(), checked: false },
        MultiSelectItem { name: "autostart".into(), label: "Autostart — launch at system startup".into(), checked: false },
        MultiSelectItem { name: "log".into(), label: "Logging — structured logging".into(), checked: false },
        MultiSelectItem { name: "sql".into(), label: "SQLite — embedded database".into(), checked: false },
        MultiSelectItem { name: "fs".into(), label: "Filesystem — file read/write access".into(), checked: false },
        MultiSelectItem { name: "shell".into(), label: "Shell — execute system commands".into(), checked: false },
        MultiSelectItem { name: "http".into(), label: "HTTP Client — make HTTP requests".into(), checked: false },
        MultiSelectItem { name: "deep-link".into(), label: "Deep Links — custom URL protocol".into(), checked: false },
        MultiSelectItem { name: "window-state".into(), label: "Window State — persist size/position".into(), checked: false },
        MultiSelectItem { name: "cmdk".into(), label: "Command Palette — Ctrl+K search".into(), checked: false },
        MultiSelectItem { name: "i18n".into(), label: "i18n — internationalization".into(), checked: false },
        MultiSelectItem { name: "tanstack-query".into(), label: "TanStack Query — data fetching".into(), checked: false },
        MultiSelectItem { name: "framer-motion".into(), label: "Motion — animations & transitions".into(), checked: false },
        MultiSelectItem { name: "react-hook-form".into(), label: "React Hook Form + Zod".into(), checked: false },
        MultiSelectItem { name: "tanstack-router".into(), label: "TanStack Router — type-safe routing".into(), checked: false },
        MultiSelectItem { name: "date-fns".into(), label: "date-fns — date utilities".into(), checked: false },
        MultiSelectItem { name: "sentry".into(), label: "Sentry — error tracking".into(), checked: false },
    ];
    let extras_select = MultiSelectState::new(extras_items).with_preselected(pre_extras);

    let name_val = pre_name.unwrap_or("").to_string();
    let slug_val = if let Some(s) = pre_slug {
        s.to_string()
    } else if !name_val.is_empty() {
        tokens::to_slug(&name_val)
    } else {
        String::new()
    };
    let bundle_val = if let Some(b) = pre_bundle_id {
        b.to_string()
    } else if !slug_val.is_empty() {
        tokens::to_bundle_id(&slug_val)
    } else {
        String::new()
    };

    let metadata = TextInputState::new(vec![
        TextField::new("App name", "My App").with_value(&name_val),
        TextField::new("Slug", "my-app").with_value(&slug_val).with_derived(true),
        TextField::new("Bundle ID", "com.example.my-app").with_value(&bundle_val).with_derived(true),
        TextField::new("Version", "0.1.0").with_value(pre_version.unwrap_or("0.1.0")),
        TextField::new("Author", "").with_value(pre_author.unwrap_or("")),
        TextField::new("Description", "").with_value(pre_description.unwrap_or("")),
    ]);

    let mut state = WizardState {
        screen: Screen::PackageManager,
        pm_select,
        auth_select,
        ui_select,
        module_phase: ModulePhase::Auth,
        extras_select,
        metadata,
        tick: 0,
        content_area: Rect::default(),
    };

    let mut terminal =
        super::enter_tui().map_err(|e| anyhow::anyhow!("Failed to enter TUI: {e}"))?;

    let tick_rate = Duration::from_millis(80);
    let confirmed;

    loop {
        terminal
            .draw(|f| render(&mut state, f))
            .map_err(|e| anyhow::anyhow!("Render error: {e}"))?;

        if event::poll(tick_rate).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) => {
                    if key.kind == KeyEventKind::Release {
                        state.tick += 1;
                        continue;
                    }

                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        super::leave_tui()
                            .map_err(|e| anyhow::anyhow!("Failed to leave TUI: {e}"))?;
                        return Ok(None);
                    }

                    match handle_input(&mut state, key.code) {
                        InputResult::Continue => {}
                        InputResult::Quit => {
                            super::leave_tui()
                                .map_err(|e| anyhow::anyhow!("Failed to leave TUI: {e}"))?;
                            return Ok(None);
                        }
                        InputResult::Confirm => {
                            confirmed = true;
                            break;
                        }
                    }
                }
                Ok(Event::Mouse(mouse)) => {
                    handle_mouse(&mut state, mouse.kind, mouse.column, mouse.row);
                }
                _ => {}
            }
        }

        state.tick += 1;
    }

    super::leave_tui().map_err(|e| anyhow::anyhow!("Failed to leave TUI: {e}"))?;

    if !confirmed {
        return Ok(None);
    }

    let app_name = {
        let v = state.metadata.value(0);
        if v.is_empty() { "My App".to_string() } else { v.to_string() }
    };
    let slug = {
        let v = state.metadata.value(1);
        if v.is_empty() { tokens::to_slug(&app_name) } else { v.to_string() }
    };
    let bundle_id = {
        let v = state.metadata.value(2);
        if v.is_empty() { tokens::to_bundle_id(&slug) } else { v.to_string() }
    };
    let version = {
        let v = state.metadata.value(3);
        if v.is_empty() { "0.1.0".to_string() } else { v.to_string() }
    };

    Ok(Some(WizardResult {
        pm: state.pm_select.selected_label().to_string(),
        auth: state.auth_select.selected_label().to_string(),
        ui: state.ui_select.selected_label().to_string(),
        extras: state.extras_select.selected_names(),
        app_name,
        slug,
        bundle_id,
        version,
        author: state.metadata.value(4).to_string(),
        description: state.metadata.value(5).to_string(),
    }))
}

enum InputResult {
    Continue,
    Quit,
    Confirm,
}

fn handle_input(state: &mut WizardState, key: KeyCode) -> InputResult {
    match state.screen {
        Screen::PackageManager => match key {
            KeyCode::Up | KeyCode::Char('k') => state.pm_select.up(),
            KeyCode::Down | KeyCode::Char('j') => state.pm_select.down(),
            KeyCode::Enter => state.screen = Screen::Modules,
            KeyCode::Esc | KeyCode::Char('q') => return InputResult::Quit,
            _ => {}
        },
        Screen::Modules => match state.module_phase {
            ModulePhase::Auth => match key {
                KeyCode::Up | KeyCode::Char('k') => state.auth_select.up(),
                KeyCode::Down | KeyCode::Char('j') => state.auth_select.down(),
                KeyCode::Enter => state.module_phase = ModulePhase::Ui,
                KeyCode::Esc => state.screen = Screen::PackageManager,
                _ => {}
            },
            ModulePhase::Ui => match key {
                KeyCode::Up | KeyCode::Char('k') => state.ui_select.up(),
                KeyCode::Down | KeyCode::Char('j') => state.ui_select.down(),
                KeyCode::Enter => state.screen = Screen::Extras,
                KeyCode::Esc => state.module_phase = ModulePhase::Auth,
                _ => {}
            },
        },
        Screen::Extras => {
            if state.extras_select.filtering {
                match key {
                    KeyCode::Esc => state.extras_select.clear_filter(),
                    KeyCode::Backspace => {
                        if state.extras_select.filter.is_empty() {
                            state.extras_select.clear_filter();
                        } else {
                            state.extras_select.filter_backspace();
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => state.extras_select.up(),
                    KeyCode::Down | KeyCode::Char('j') => state.extras_select.down(),
                    KeyCode::Char(' ') => state.extras_select.toggle(),
                    KeyCode::Enter => state.extras_select.clear_filter(),
                    KeyCode::Char(c) => state.extras_select.filter_char(c),
                    _ => {}
                }
            } else {
                match key {
                    KeyCode::Up | KeyCode::Char('k') => state.extras_select.up(),
                    KeyCode::Down | KeyCode::Char('j') => state.extras_select.down(),
                    KeyCode::Char(' ') => state.extras_select.toggle(),
                    KeyCode::Char('/') => state.extras_select.start_filter(),
                    KeyCode::Enter => state.screen = Screen::Metadata,
                    KeyCode::Esc => {
                        state.module_phase = ModulePhase::Ui;
                        state.screen = Screen::Modules;
                    }
                    _ => {}
                }
            }
        }
        Screen::Metadata => match key {
            KeyCode::Tab => {
                update_derived_fields(state);
                state.metadata.next_field();
            }
            KeyCode::BackTab => state.metadata.prev_field(),
            KeyCode::Enter => {
                if state.metadata.active + 1 < state.metadata.fields.len() {
                    update_derived_fields(state);
                    state.metadata.next_field();
                } else {
                    update_derived_fields(state);
                    state.screen = Screen::Summary;
                }
            }
            KeyCode::Esc => state.screen = Screen::Extras,
            KeyCode::Backspace => state.metadata.delete_char(),
            KeyCode::Left => state.metadata.move_left(),
            KeyCode::Right => state.metadata.move_right(),
            KeyCode::Char(c) => state.metadata.insert_char(c),
            _ => {}
        },
        Screen::Summary => match key {
            KeyCode::Enter => return InputResult::Confirm,
            KeyCode::Esc => state.screen = Screen::Metadata,
            KeyCode::Char('q') => return InputResult::Quit,
            _ => {}
        },
    }
    InputResult::Continue
}

fn handle_mouse(state: &mut WizardState, kind: MouseEventKind, _col: u16, row: u16) {
    match kind {
        MouseEventKind::ScrollUp => match state.screen {
            Screen::PackageManager => state.pm_select.up(),
            Screen::Modules => match state.module_phase {
                ModulePhase::Auth => state.auth_select.up(),
                ModulePhase::Ui => state.ui_select.up(),
            },
            Screen::Extras => state.extras_select.up(),
            _ => {}
        },
        MouseEventKind::ScrollDown => match state.screen {
            Screen::PackageManager => state.pm_select.down(),
            Screen::Modules => match state.module_phase {
                ModulePhase::Auth => state.auth_select.down(),
                ModulePhase::Ui => state.ui_select.down(),
            },
            Screen::Extras => state.extras_select.down(),
            _ => {}
        },
        MouseEventKind::Down(MouseButton::Left) => {
            let area = state.content_area;
            if row < area.y || row >= area.y + area.height {
                return;
            }
            let relative_row = (row - area.y) as usize;

            match state.screen {
                Screen::PackageManager => state.pm_select.set_cursor(relative_row),
                Screen::Modules => match state.module_phase {
                    ModulePhase::Auth => state.auth_select.set_cursor(relative_row),
                    ModulePhase::Ui => state.ui_select.set_cursor(relative_row),
                },
                Screen::Extras => {
                    // Account for counter line + blank line before the list
                    let list_offset = 2;
                    if relative_row >= list_offset {
                        let idx = relative_row - list_offset + state.extras_select.scroll_offset;
                        state.extras_select.set_cursor(idx);
                        state.extras_select.toggle();
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn update_derived_fields(state: &mut WizardState) {
    let name = state.metadata.value(0).to_string();
    if !name.is_empty() {
        let slug_field = &state.metadata.fields[1];
        if slug_field.derived && (slug_field.value.is_empty() || slug_field.value == tokens::to_slug(&name.replace(|c: char| c.is_ascii_uppercase(), ""))) {
            let new_slug = tokens::to_slug(&name);
            state.metadata.fields[1].value = new_slug.clone();
            state.metadata.fields[1].cursor = state.metadata.fields[1].value.len();

            if state.metadata.fields[2].derived {
                let new_bundle = tokens::to_bundle_id(&new_slug);
                state.metadata.fields[2].value = new_bundle;
                state.metadata.fields[2].cursor = state.metadata.fields[2].value.len();
            }
        }
    }
}

fn render(state: &mut WizardState, frame: &mut ratatui::Frame) {
    let area = frame.area();

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(theme::border())
        .style(Style::default().bg(theme::BG));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let chunks = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Min(8),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .split(inner);

    render_header(state, frame, chunks[0]);
    render_separator(frame, chunks[1]);
    render_content(state, frame, chunks[2]);
    render_separator(frame, chunks[3]);
    render_footer(state, frame, chunks[4]);
}

fn render_header(state: &WizardState, frame: &mut ratatui::Frame, area: Rect) {
    let header = Block::default()
        .padding(Padding::horizontal(2))
        .style(Style::default().bg(theme::BG));
    let inner = header.inner(area);
    frame.render_widget(header, area);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let title = Line::from(vec![
        Span::styled("Crabyard", Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  v{}", env!("CARGO_PKG_VERSION")), theme::subtitle()),
    ]);
    frame.render_widget(Paragraph::new(title), rows[0]);

    let screen_label = match state.screen {
        Screen::PackageManager => "Package Manager",
        Screen::Modules => match state.module_phase {
            ModulePhase::Auth => "Auth Provider",
            ModulePhase::Ui => "UI Framework",
        },
        Screen::Extras => "Extras",
        Screen::Metadata => "App Details",
        Screen::Summary => "Confirm",
    };
    let step = state.screen.index() + 1;
    let total = Screen::total();

    let progress_line = Line::from(vec![
        Span::styled(screen_label, theme::accent()),
        Span::styled(format!("  {step}/{total}"), Style::default().fg(theme::DIM)),
    ]);
    frame.render_widget(Paragraph::new(progress_line), rows[1]);

    let bar_width = (inner.width as usize).saturating_sub(2);
    let filled = (bar_width * step) / total;
    let empty = bar_width - filled;
    let bar = Line::from(vec![
        Span::styled("━".repeat(filled), theme::accent()),
        Span::styled("━".repeat(empty), Style::default().fg(theme::BORDER_COLOR)),
    ]);
    frame.render_widget(Paragraph::new(bar), rows[2]);
}

fn render_separator(frame: &mut ratatui::Frame, area: Rect) {
    let sep = "─".repeat(area.width as usize);
    frame.render_widget(Paragraph::new(sep).style(theme::border()), area);
}

fn render_content(state: &mut WizardState, frame: &mut ratatui::Frame, area: Rect) {
    let content = Block::default()
        .padding(Padding::new(2, 2, 1, 0))
        .style(Style::default().bg(theme::BG));
    let inner = content.inner(area);
    frame.render_widget(content, area);

    state.content_area = inner;

    match state.screen {
        Screen::PackageManager => state.pm_select.render(frame, inner),
        Screen::Modules => match state.module_phase {
            ModulePhase::Auth => state.auth_select.render(frame, inner),
            ModulePhase::Ui => state.ui_select.render(frame, inner),
        },
        Screen::Extras => {
            let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
                .split(inner);

            let count = state.extras_select.selected_count();
            let counter = Line::from(vec![
                Span::styled(
                    format!("{count} selected"),
                    if count > 0 { theme::accent() } else { theme::subtitle() },
                ),
                Span::styled("  Space to toggle", Style::default().fg(theme::DIM)),
            ]);
            frame.render_widget(Paragraph::new(counter), rows[0]);
            frame.render_widget(Paragraph::new(""), rows[1]);

            state.extras_select.render(frame, rows[2]);
        }
        Screen::Metadata => {
            state.metadata.render(frame, inner, state.tick);
        }
        Screen::Summary => {
            render_summary(state, frame, inner);
        }
    }
}

fn render_summary(state: &WizardState, frame: &mut ratatui::Frame, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    let pairs: Vec<(&str, String)> = vec![
        ("Package manager", state.pm_select.selected_label().to_string()),
        ("Auth provider", state.auth_select.selected_label().to_string()),
        ("UI framework", state.ui_select.selected_label().to_string()),
        ("Extras", {
            let count = state.extras_select.selected_count();
            if count == 0 { "none".into() } else { format!("{count} selected") }
        }),
        ("App name", {
            let v = state.metadata.value(0);
            if v.is_empty() { "My App".into() } else { v.into() }
        }),
        ("Slug", {
            let v = state.metadata.value(1);
            if v.is_empty() { "my-app".into() } else { v.into() }
        }),
        ("Bundle ID", {
            let v = state.metadata.value(2);
            if v.is_empty() { "com.example.my-app".into() } else { v.into() }
        }),
        ("Version", {
            let v = state.metadata.value(3);
            if v.is_empty() { "0.1.0".into() } else { v.into() }
        }),
    ];

    let author = state.metadata.value(4);
    let desc = state.metadata.value(5);

    for (label, value) in &pairs {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18}", label), theme::subtitle()),
            Span::styled(value.as_str(), theme::accent()),
        ]));
    }

    if !author.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18}", "Author"), theme::subtitle()),
            Span::styled(author, theme::accent()),
        ]));
    }
    if !desc.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18}", "Description"), theme::subtitle()),
            Span::styled(desc, theme::accent()),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("  Press ", Style::default().fg(theme::DIM)),
        Span::styled("Enter", theme::accent()),
        Span::styled(" to generate", Style::default().fg(theme::DIM)),
    ]));

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_footer(state: &WizardState, frame: &mut ratatui::Frame, area: Rect) {
    let footer = Block::default()
        .padding(Padding::horizontal(2))
        .style(Style::default().bg(theme::BG));
    let inner = footer.inner(area);
    frame.render_widget(footer, area);

    let hints = match state.screen {
        Screen::PackageManager => "↑↓ select  Enter next  Esc quit",
        Screen::Modules => "↑↓ select  Enter next  Esc back",
        Screen::Extras => "↑↓ move  Space toggle  / filter  Enter next  Esc back",
        Screen::Metadata => "Tab next field  Enter next  Esc back",
        Screen::Summary => "Enter generate  Esc back  q quit",
    };

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hints, theme::hint()))),
        inner,
    );
}
