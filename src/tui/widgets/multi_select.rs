use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::theme;

pub struct MultiSelectState {
    pub items: Vec<MultiSelectItem>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub filter: String,
    pub filtering: bool,
}

pub struct MultiSelectItem {
    pub name: String,
    pub label: String,
    pub checked: bool,
}

impl MultiSelectState {
    pub fn new(items: Vec<MultiSelectItem>) -> Self {
        Self {
            items,
            cursor: 0,
            scroll_offset: 0,
            filter: String::new(),
            filtering: false,
        }
    }

    pub fn with_preselected(mut self, names: &[String]) -> Self {
        for item in &mut self.items {
            if names.contains(&item.name) {
                item.checked = true;
            }
        }
        self
    }

    fn filtered_indices(&self) -> Vec<usize> {
        if self.filter.is_empty() {
            return (0..self.items.len()).collect();
        }
        let lower = self.filter.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.label.to_lowercase().contains(&lower))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn start_filter(&mut self) {
        self.filtering = true;
    }

    pub fn filter_char(&mut self, ch: char) {
        self.filter.push(ch);
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    pub fn filter_backspace(&mut self) {
        self.filter.pop();
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.filtering = false;
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    pub fn up(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            return;
        }
        if self.cursor > 0 {
            self.cursor -= 1;
            if self.cursor < self.scroll_offset {
                self.scroll_offset = self.cursor;
            }
        }
    }

    pub fn down(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            return;
        }
        if self.cursor + 1 < indices.len() {
            self.cursor += 1;
        }
    }

    pub fn set_cursor(&mut self, idx: usize) {
        let indices = self.filtered_indices();
        if idx < indices.len() {
            self.cursor = idx;
        }
    }

    pub fn toggle(&mut self) {
        let indices = self.filtered_indices();
        if let Some(&real_idx) = indices.get(self.cursor) {
            self.items[real_idx].checked = !self.items[real_idx].checked;
        }
    }

    pub fn selected_names(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|i| i.checked)
            .map(|i| i.name.clone())
            .collect()
    }

    pub fn selected_count(&self) -> usize {
        self.items.iter().filter(|i| i.checked).count()
    }

    pub fn render(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let indices = self.filtered_indices();

        if self.filtering {
            let filter_area = Rect { height: 1, ..area };
            let list_area = Rect {
                y: area.y + 1,
                height: area.height.saturating_sub(1),
                ..area
            };

            let filter_line = Line::from(vec![
                Span::styled("/ ", theme::accent()),
                Span::styled(&self.filter, theme::text()),
                Span::styled("_", Style::default().fg(theme::MUTED)),
            ]);
            frame.render_widget(Paragraph::new(filter_line), filter_area);

            self.render_items(frame, list_area, &indices);
        } else {
            self.render_items(frame, area, &indices);
        }
    }

    fn render_items(&mut self, frame: &mut ratatui::Frame, area: Rect, indices: &[usize]) {
        if indices.is_empty() {
            let msg = Line::from(Span::styled("  No matches", Style::default().fg(theme::DIM)));
            frame.render_widget(Paragraph::new(msg), area);
            return;
        }

        let visible = area.height as usize;

        if self.cursor >= self.scroll_offset + visible {
            self.scroll_offset = self.cursor + 1 - visible;
        }
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        }

        let end = (self.scroll_offset + visible).min(indices.len());
        let visible_slice = &indices[self.scroll_offset..end];

        let lines: Vec<Line> = visible_slice
            .iter()
            .enumerate()
            .map(|(vi, &real_idx)| {
                let cursor_idx = vi + self.scroll_offset;
                let active = cursor_idx == self.cursor;
                let item = &self.items[real_idx];
                let check = if item.checked { "◉" } else { "○" };
                let indicator = if active { "› " } else { "  " };

                let check_style = if item.checked {
                    theme::accent()
                } else if active {
                    Style::default().fg(theme::MUTED)
                } else {
                    Style::default().fg(theme::DIM)
                };

                let label_style = if active {
                    theme::selected()
                } else if item.checked {
                    theme::text()
                } else {
                    Style::default().fg(theme::MUTED)
                };

                Line::from(vec![
                    Span::styled(indicator, label_style),
                    Span::styled(format!("{check} "), check_style),
                    Span::styled(&item.label, label_style),
                ])
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_items(n: usize) -> Vec<MultiSelectItem> {
        (0..n)
            .map(|i| MultiSelectItem {
                name: format!("feat-{i}"),
                label: format!("Feature {i} — description {i}"),
                checked: false,
            })
            .collect()
    }

    #[test]
    fn initial_state() {
        let ms = MultiSelectState::new(make_items(3));
        assert_eq!(ms.cursor, 0);
        assert_eq!(ms.selected_count(), 0);
        assert!(ms.selected_names().is_empty());
    }

    #[test]
    fn toggle_selects_item() {
        let mut ms = MultiSelectState::new(make_items(3));
        ms.toggle();
        assert_eq!(ms.selected_count(), 1);
        assert_eq!(ms.selected_names(), vec!["feat-0"]);
    }

    #[test]
    fn toggle_deselects_item() {
        let mut ms = MultiSelectState::new(make_items(3));
        ms.toggle();
        ms.toggle();
        assert_eq!(ms.selected_count(), 0);
    }

    #[test]
    fn down_up_navigation() {
        let mut ms = MultiSelectState::new(make_items(5));
        ms.down();
        ms.down();
        assert_eq!(ms.cursor, 2);
        ms.up();
        assert_eq!(ms.cursor, 1);
    }

    #[test]
    fn down_clamps_at_end() {
        let mut ms = MultiSelectState::new(make_items(3));
        ms.down();
        ms.down();
        ms.down();
        assert_eq!(ms.cursor, 2);
    }

    #[test]
    fn up_clamps_at_zero() {
        let mut ms = MultiSelectState::new(make_items(3));
        ms.up();
        assert_eq!(ms.cursor, 0);
    }

    #[test]
    fn toggle_at_cursor_position() {
        let mut ms = MultiSelectState::new(make_items(3));
        ms.down();
        ms.toggle();
        assert_eq!(ms.selected_names(), vec!["feat-1"]);
    }

    #[test]
    fn with_preselected() {
        let ms = MultiSelectState::new(make_items(4))
            .with_preselected(&["feat-1".into(), "feat-3".into()]);
        assert_eq!(ms.selected_count(), 2);
        assert_eq!(ms.selected_names(), vec!["feat-1", "feat-3"]);
    }

    #[test]
    fn filter_narrows_list() {
        let mut ms = MultiSelectState::new(make_items(5));
        ms.start_filter();
        ms.filter_char('0');
        let indices = ms.filtered_indices();
        assert_eq!(indices.len(), 1); // Only "Feature 0"
    }

    #[test]
    fn filter_backspace_widens() {
        let mut ms = MultiSelectState::new(make_items(5));
        ms.start_filter();
        ms.filter_char('x');
        assert_eq!(ms.filtered_indices().len(), 0);
        ms.filter_backspace();
        assert_eq!(ms.filtered_indices().len(), 5);
    }

    #[test]
    fn clear_filter_resets() {
        let mut ms = MultiSelectState::new(make_items(5));
        ms.start_filter();
        ms.filter_char('0');
        ms.clear_filter();
        assert!(!ms.filtering);
        assert!(ms.filter.is_empty());
        assert_eq!(ms.filtered_indices().len(), 5);
    }

    #[test]
    fn toggle_through_filter() {
        let mut ms = MultiSelectState::new(make_items(5));
        ms.start_filter();
        ms.filter_char('3');
        // Only "Feature 3" should match, cursor at 0 in filtered view
        ms.toggle();
        assert_eq!(ms.selected_names(), vec!["feat-3"]);
    }

    #[test]
    fn set_cursor_in_bounds() {
        let mut ms = MultiSelectState::new(make_items(5));
        ms.set_cursor(3);
        assert_eq!(ms.cursor, 3);
    }

    #[test]
    fn set_cursor_out_of_bounds() {
        let mut ms = MultiSelectState::new(make_items(3));
        ms.set_cursor(10);
        assert_eq!(ms.cursor, 0); // unchanged
    }
}
