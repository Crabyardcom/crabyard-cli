use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Gauge, Padding, Paragraph, Wrap};

use super::theme;
use super::{ferris, poll_quit};

#[derive(Clone, Copy, PartialEq)]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Skipped,
    Failed,
}

#[derive(Clone)]
pub struct Step {
    pub label: String,
    pub status: StepStatus,
}

pub enum WorkerMsg {
    StepDone(usize),
    StepFailed(usize, String),
    AllDone,
}

pub fn run_generation<F>(app_slug: &str, steps: Vec<Step>, work: F) -> anyhow::Result<()>
where
    F: FnOnce(mpsc::Sender<WorkerMsg>) -> anyhow::Result<()> + Send + 'static,
{
    let mut terminal = super::enter_tui().map_err(|e| anyhow::anyhow!("Failed to enter TUI: {e}"))?;
    let (tx, rx) = mpsc::channel::<WorkerMsg>();

    let work_result: std::sync::Arc<std::sync::Mutex<Option<anyhow::Result<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let work_result_clone = work_result.clone();

    thread::spawn(move || {
        let result = work(tx);
        *work_result_clone.lock().unwrap() = Some(result);
    });

    let mut view = GenerationView::new(app_slug.to_string(), steps);
    let tick_rate = Duration::from_millis(100);
    let min_step_time = Duration::from_millis(350);
    let mut last_step_time = Instant::now();
    let mut pending_msgs: Vec<WorkerMsg> = Vec::new();
    let mut all_done = false;

    loop {
        terminal
            .draw(|f| view.render(f))
            .map_err(|e| anyhow::anyhow!("Render error: {e}"))?;

        if poll_quit(tick_rate) {
            break;
        }

        view.tick += 1;

        while let Ok(msg) = rx.try_recv() {
            pending_msgs.push(msg);
        }

        if !pending_msgs.is_empty() && last_step_time.elapsed() >= min_step_time {
            let msg = pending_msgs.remove(0);
            match msg {
                WorkerMsg::StepDone(i) => {
                    view.complete_step(i);
                    last_step_time = Instant::now();
                }
                WorkerMsg::StepFailed(i, err) => {
                    view.fail_step(i, Some(err));
                    last_step_time = Instant::now();
                }
                WorkerMsg::AllDone => {
                    all_done = true;
                }
            }
        }

        if all_done && pending_msgs.is_empty() {
            view.celebrating = true;
            terminal
                .draw(|f| view.render(f))
                .map_err(|e| anyhow::anyhow!("Render error: {e}"))?;
            thread::sleep(Duration::from_millis(1800));
            break;
        }

        if !all_done && pending_msgs.is_empty() {
            if let Ok(guard) = work_result.try_lock() {
                if guard.is_some() {
                    drop(guard);
                    break;
                }
            }
        }
    }

    super::leave_tui().map_err(|e| anyhow::anyhow!("Failed to leave TUI: {e}"))?;

    let result = work_result.lock().unwrap().take();
    match result {
        Some(r) => r,
        None => Ok(()),
    }
}

struct GenerationView {
    app_slug: String,
    steps: Vec<Step>,
    tick: usize,
    celebrating: bool,
    last_error: Option<String>,
}

impl GenerationView {
    fn new(app_slug: String, steps: Vec<Step>) -> Self {
        Self {
            app_slug,
            steps,
            tick: 0,
            celebrating: false,
            last_error: None,
        }
    }

    fn complete_step(&mut self, idx: usize) {
        if let Some(s) = self.steps.get_mut(idx) {
            s.status = StepStatus::Done;
        }
        if let Some(next) = self.steps.get_mut(idx + 1) {
            if next.status == StepStatus::Pending {
                next.status = StepStatus::Running;
            }
        }
    }

    #[allow(dead_code)]
    fn skip_step(&mut self, idx: usize) {
        if let Some(s) = self.steps.get_mut(idx) {
            s.status = StepStatus::Skipped;
        }
        if let Some(next) = self.steps.get_mut(idx + 1) {
            if next.status == StepStatus::Pending {
                next.status = StepStatus::Running;
            }
        }
    }

    fn fail_step(&mut self, idx: usize, err: Option<String>) {
        if let Some(s) = self.steps.get_mut(idx) {
            s.status = StepStatus::Failed;
        }
        if err.is_some() {
            self.last_error = err;
        }
    }

    fn done_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s.status, StepStatus::Done | StepStatus::Skipped))
            .count()
    }

    fn render(&self, frame: &mut ratatui::Frame) {
        let area = frame.area();

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(if self.celebrating {
                theme::border_highlight()
            } else {
                theme::border()
            })
            .style(Style::default().bg(theme::BG));
        let inner = outer_block.inner(area);
        frame.render_widget(outer_block, area);

        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(12),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(inner);

        self.render_header(frame, chunks[0]);
        self.render_separator(frame, chunks[1]);
        self.render_body(frame, chunks[2]);
        self.render_separator(frame, chunks[3]);
        self.render_progress(frame, chunks[4]);
    }

    fn render_header(&self, frame: &mut ratatui::Frame, area: Rect) {
        let header_block = Block::default()
            .padding(Padding::horizontal(2))
            .style(Style::default().bg(theme::BG));
        let header_inner = header_block.inner(area);
        frame.render_widget(header_block, area);

        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)])
            .split(header_inner);

        let title = Line::from(vec![
            Span::styled("  Crabyard ", theme::title()),
            Span::styled(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                theme::subtitle(),
            ),
        ]);
        frame.render_widget(Paragraph::new(title), chunks[0]);

        let subtitle_text = if self.celebrating {
            "  Done!"
        } else {
            "  Building"
        };
        let sub = Line::from(vec![
            Span::styled(subtitle_text, theme::text()),
            if !self.celebrating {
                Span::styled(
                    format!(" {}", self.app_slug),
                    theme::accent(),
                )
            } else {
                Span::raw("")
            },
        ]);
        frame.render_widget(Paragraph::new(sub), chunks[1]);
    }

    fn render_separator(&self, frame: &mut ratatui::Frame, area: Rect) {
        let sep = "─".repeat(area.width as usize);
        frame.render_widget(
            Paragraph::new(sep).style(theme::border()),
            area,
        );
    }

    fn render_body(&self, frame: &mut ratatui::Frame, area: Rect) {
        let body_block = Block::default()
            .padding(Padding::horizontal(1))
            .style(Style::default().bg(theme::BG));
        let body_inner = body_block.inner(area);
        frame.render_widget(body_block, area);

        let cols = Layout::horizontal([Constraint::Length(28), Constraint::Min(30)])
            .split(body_inner);

        self.render_ferris(frame, cols[0]);
        self.render_steps(frame, cols[1]);
    }

    fn render_ferris(&self, frame: &mut ratatui::Frame, area: Rect) {
        let bob = ferris::bob_offset(self.tick, self.celebrating);

        let mut lines: Vec<Line> = Vec::new();
        for _ in 0..bob {
            lines.push(Line::raw(""));
        }
        for (i, l) in ferris::ART.lines().enumerate() {
            let color = ferris::line_color(i, self.tick, self.celebrating);
            lines.push(Line::from(Span::styled(l, Style::default().fg(color))));
        }

        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false })
                .style(Style::default().bg(theme::BG)),
            area,
        );
    }

    fn render_steps(&self, frame: &mut ratatui::Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        for step in &self.steps {
            let line = match step.status {
                StepStatus::Pending => {
                    let style = theme::pending();
                    Line::from(vec![
                        Span::styled("  ○", style),
                        Span::raw(" "),
                        Span::styled(&step.label, style),
                    ])
                }
                StepStatus::Running => {
                    let dots = match (self.tick / 4) % 4 {
                        0 => "   ",
                        1 => ".  ",
                        2 => ".. ",
                        _ => "...",
                    };
                    Line::from(vec![
                        Span::styled("  ▸ ", theme::running()),
                        Span::styled(&step.label, theme::running()),
                        Span::styled(dots, theme::running()),
                    ])
                }
                StepStatus::Done => {
                    let style = theme::success();
                    Line::from(vec![
                        Span::styled("  ✓", style),
                        Span::raw(" "),
                        Span::styled(&step.label, style),
                    ])
                }
                StepStatus::Skipped => {
                    let style = theme::pending();
                    Line::from(vec![
                        Span::styled("  ⊘", style),
                        Span::raw(" "),
                        Span::styled(&step.label, style),
                    ])
                }
                StepStatus::Failed => {
                    let style = theme::error();
                    Line::from(vec![
                        Span::styled("  ✗", style),
                        Span::raw(" "),
                        Span::styled(&step.label, style),
                    ])
                }
            };
            lines.push(line);
        }

        if let Some(err) = &self.last_error {
            lines.push(Line::raw(""));
            let truncated: String = err.chars().take(60).collect();
            lines.push(Line::from(Span::styled(
                format!("    {truncated}"),
                theme::error(),
            )));
        }

        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().bg(theme::BG)),
            area,
        );
    }

    fn render_progress(&self, frame: &mut ratatui::Frame, area: Rect) {
        let progress_block = Block::default()
            .padding(Padding::horizontal(2))
            .style(Style::default().bg(theme::BG));
        let progress_inner = progress_block.inner(area);
        frame.render_widget(progress_block, area);

        let total = self.steps.len();
        let done = self.done_count();
        let ratio = if total > 0 {
            done as f64 / total as f64
        } else {
            0.0
        };

        let label = if self.celebrating {
            format!("Done! {done}/{total} steps completed")
        } else {
            format!("{done}/{total} steps")
        };

        let gauge = Gauge::default()
            .gauge_style(theme::progress_filled())
            .label(Span::styled(label, theme::text()))
            .ratio(ratio);
        frame.render_widget(gauge, progress_inner);
    }
}
