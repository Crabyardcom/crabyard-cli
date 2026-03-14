use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::theme;

pub struct SelectState {
    pub items: Vec<SelectItem>,
    pub cursor: usize,
}

pub struct SelectItem {
    pub label: String,
    pub description: String,
}

impl SelectState {
    pub fn new(items: Vec<SelectItem>) -> Self {
        Self { items, cursor: 0 }
    }

    pub fn up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.cursor + 1 < self.items.len() {
            self.cursor += 1;
        }
    }

    pub fn set_cursor(&mut self, idx: usize) {
        if idx < self.items.len() {
            self.cursor = idx;
        }
    }

    pub fn selected_label(&self) -> &str {
        &self.items[self.cursor].label
    }

    pub fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
        let lines: Vec<Line> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let active = i == self.cursor;
                let indicator = if active { "› " } else { "  " };
                let label_style = if active {
                    theme::selected()
                } else {
                    theme::text()
                };
                let desc_style = if active {
                    Style::default().fg(theme::MUTED)
                } else {
                    Style::default().fg(theme::DIM)
                };
                Line::from(vec![
                    Span::styled(indicator, label_style),
                    Span::styled(&item.label, label_style),
                    Span::styled(format!("  {}", item.description), desc_style),
                ])
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_select(n: usize) -> SelectState {
        let items = (0..n)
            .map(|i| SelectItem {
                label: format!("item-{i}"),
                description: format!("desc-{i}"),
            })
            .collect();
        SelectState::new(items)
    }

    #[test]
    fn initial_cursor_is_zero() {
        let s = make_select(3);
        assert_eq!(s.cursor, 0);
        assert_eq!(s.selected_label(), "item-0");
    }

    #[test]
    fn down_moves_cursor() {
        let mut s = make_select(3);
        s.down();
        assert_eq!(s.cursor, 1);
        assert_eq!(s.selected_label(), "item-1");
    }

    #[test]
    fn down_clamps_at_end() {
        let mut s = make_select(3);
        s.down();
        s.down();
        s.down(); // should not go past 2
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn up_clamps_at_zero() {
        let mut s = make_select(3);
        s.up(); // already at 0
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn up_moves_cursor() {
        let mut s = make_select(3);
        s.down();
        s.down();
        s.up();
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn set_cursor_in_bounds() {
        let mut s = make_select(4);
        s.set_cursor(2);
        assert_eq!(s.cursor, 2);
        assert_eq!(s.selected_label(), "item-2");
    }

    #[test]
    fn set_cursor_out_of_bounds_ignored() {
        let mut s = make_select(3);
        s.set_cursor(10);
        assert_eq!(s.cursor, 0); // unchanged
    }

    #[test]
    fn item_count() {
        let s = make_select(5);
        assert_eq!(s.items.len(), 5);
    }
}
