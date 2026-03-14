use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::theme;

pub struct TextInputState {
    pub fields: Vec<TextField>,
    pub active: usize,
}

pub struct TextField {
    pub label: String,
    pub value: String,
    pub placeholder: String,
    pub cursor: usize,
    pub derived: bool,
}

impl TextField {
    pub fn new(label: &str, placeholder: &str) -> Self {
        Self {
            label: label.to_string(),
            value: String::new(),
            placeholder: placeholder.to_string(),
            cursor: 0,
            derived: false,
        }
    }

    pub fn with_value(mut self, value: &str) -> Self {
        self.value = value.to_string();
        self.cursor = value.len();
        self
    }

    pub fn with_derived(mut self, derived: bool) -> Self {
        self.derived = derived;
        self
    }
}

impl TextInputState {
    pub fn new(fields: Vec<TextField>) -> Self {
        Self { fields, active: 0 }
    }

    pub fn next_field(&mut self) {
        if self.active + 1 < self.fields.len() {
            self.active += 1;
        }
    }

    pub fn prev_field(&mut self) {
        if self.active > 0 {
            self.active -= 1;
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        let field = &mut self.fields[self.active];
        field.value.insert(field.cursor, ch);
        field.cursor += ch.len_utf8();
    }

    pub fn delete_char(&mut self) {
        let field = &mut self.fields[self.active];
        if field.cursor > 0 {
            let prev = field.value[..field.cursor]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            field.cursor -= prev;
            field.value.remove(field.cursor);
        }
    }

    pub fn move_left(&mut self) {
        let field = &mut self.fields[self.active];
        if field.cursor > 0 {
            let prev = field.value[..field.cursor]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            field.cursor -= prev;
        }
    }

    pub fn move_right(&mut self) {
        let field = &mut self.fields[self.active];
        if field.cursor < field.value.len() {
            let next = field.value[field.cursor..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            field.cursor += next;
        }
    }

    pub fn value(&self, index: usize) -> &str {
        &self.fields[index].value
    }

    pub fn render(&self, frame: &mut ratatui::Frame, area: Rect, tick: usize) {
        let mut lines: Vec<Line> = Vec::new();

        for (i, field) in self.fields.iter().enumerate() {
            let active = i == self.active;
            let label_style = if active {
                theme::accent()
            } else {
                theme::subtitle()
            };

            let display_val = if field.value.is_empty() && !active {
                &field.placeholder
            } else {
                &field.value
            };

            let val_style = if field.value.is_empty() && !active {
                Style::default().fg(theme::DIM)
            } else if active {
                theme::text()
            } else {
                Style::default().fg(theme::MUTED)
            };

            if active {
                let cursor_visible = (tick / 5) % 2 == 0;
                let (before, after) = display_val.split_at(field.cursor.min(display_val.len()));
                let cursor_char = if cursor_visible { "▎" } else { " " };

                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", field.label), label_style),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(before, val_style),
                    Span::styled(cursor_char, theme::accent()),
                    Span::styled(after, val_style),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", field.label), label_style),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(display_val, val_style),
                ]));
            }

            if i < self.fields.len() - 1 {
                lines.push(Line::raw(""));
            }
        }

        frame.render_widget(Paragraph::new(lines), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(n: usize) -> TextInputState {
        let fields = (0..n)
            .map(|i| TextField::new(&format!("Field {i}"), &format!("placeholder {i}")))
            .collect();
        TextInputState::new(fields)
    }

    #[test]
    fn initial_state() {
        let ti = make_input(3);
        assert_eq!(ti.active, 0);
        assert_eq!(ti.value(0), "");
    }

    #[test]
    fn insert_char() {
        let mut ti = make_input(2);
        ti.insert_char('h');
        ti.insert_char('i');
        assert_eq!(ti.value(0), "hi");
    }

    #[test]
    fn delete_char() {
        let mut ti = make_input(1);
        ti.insert_char('a');
        ti.insert_char('b');
        ti.delete_char();
        assert_eq!(ti.value(0), "a");
    }

    #[test]
    fn delete_at_start_does_nothing() {
        let mut ti = make_input(1);
        ti.delete_char();
        assert_eq!(ti.value(0), "");
    }

    #[test]
    fn move_left_right() {
        let mut ti = make_input(1);
        ti.insert_char('a');
        ti.insert_char('b');
        ti.insert_char('c');
        assert_eq!(ti.fields[0].cursor, 3);
        ti.move_left();
        assert_eq!(ti.fields[0].cursor, 2);
        ti.move_right();
        assert_eq!(ti.fields[0].cursor, 3);
    }

    #[test]
    fn move_left_clamps() {
        let mut ti = make_input(1);
        ti.move_left();
        assert_eq!(ti.fields[0].cursor, 0);
    }

    #[test]
    fn move_right_clamps() {
        let mut ti = make_input(1);
        ti.insert_char('x');
        ti.move_right();
        assert_eq!(ti.fields[0].cursor, 1); // stays at end
    }

    #[test]
    fn next_prev_field() {
        let mut ti = make_input(3);
        ti.next_field();
        assert_eq!(ti.active, 1);
        ti.next_field();
        assert_eq!(ti.active, 2);
        ti.next_field(); // clamp
        assert_eq!(ti.active, 2);
        ti.prev_field();
        assert_eq!(ti.active, 1);
        ti.prev_field();
        assert_eq!(ti.active, 0);
        ti.prev_field(); // clamp
        assert_eq!(ti.active, 0);
    }

    #[test]
    fn insert_in_middle() {
        let mut ti = make_input(1);
        ti.insert_char('a');
        ti.insert_char('c');
        ti.move_left();
        ti.insert_char('b');
        assert_eq!(ti.value(0), "abc");
    }

    #[test]
    fn with_value() {
        let field = TextField::new("Test", "ph").with_value("hello");
        assert_eq!(field.value, "hello");
        assert_eq!(field.cursor, 5);
    }

    #[test]
    fn with_derived() {
        let field = TextField::new("Test", "ph").with_derived(true);
        assert!(field.derived);
    }
}
