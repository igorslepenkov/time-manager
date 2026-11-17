use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::theme::theme;

#[derive(Clone, PartialEq)]
pub struct Input {
    pub is_focused: bool,
    pub input: String,
    /// Позиция курсора — байтовый индекс в `input`, всегда на границе символа.
    pub cursor: usize,
}

impl Input {
    pub fn init() -> Input {
        Input {
            is_focused: false,
            input: String::new(),
            cursor: 0,
        }
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
        self.cursor = 0;
        self.is_focused = false;
    }

    pub fn set_focus(&mut self) {
        self.is_focused = true;
        self.cursor = self.input.len();
    }

    pub fn unset_focus(&mut self) {
        self.is_focused = false;
    }

    /// Заменяет весь текст и ставит курсор в конец — используется при
    /// подстановке задачи из истории.
    pub fn set_input(&mut self, text: String) {
        self.input = text;
        self.cursor = self.input.len();
    }

    pub fn insert_char(&mut self, char: char) {
        self.input.insert(self.cursor, char);
        self.cursor += char.len_utf8();
    }

    /// Вставка из буфера обмена: переводы строк и прочие управляющие символы
    /// выбрасываются, потому что поле однострочное.
    pub fn insert_text(&mut self, text: &str) {
        let clean: String = text.chars().filter(|char| !char.is_control()).collect();

        self.input.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }

    pub fn backspace(&mut self) {
        if let Some(previous) = self.previous_boundary() {
            self.input.remove(previous);
            self.cursor = previous;
        }
    }

    pub fn delete_forward(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
        }
    }

    /// Ctrl+Backspace: удаляет предыдущее слово вместе с пробелами перед ним.
    pub fn delete_word_backward(&mut self) {
        let head = &self.input[..self.cursor];
        let word_end = head.trim_end_matches(char::is_whitespace).len();
        let word_start = head[..word_end]
            .trim_end_matches(|char: char| !char.is_whitespace())
            .len();
        let start = head[..word_start]
            .trim_end_matches(char::is_whitespace)
            .len();

        self.input.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    pub fn move_cursor_left(&mut self) {
        if let Some(previous) = self.previous_boundary() {
            self.cursor = previous;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if let Some(next) = self.next_boundary() {
            self.cursor = next;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor = self.input.len();
    }

    pub fn chars_before_cursor(&self) -> usize {
        self.input[..self.cursor].chars().count()
    }

    fn previous_boundary(&self) -> Option<usize> {
        if self.cursor == 0 {
            return None;
        }

        Some(
            self.input[..self.cursor]
                .char_indices()
                .last()
                .map(|(index, _)| index)
                .unwrap_or(0),
        )
    }

    fn next_boundary(&self) -> Option<usize> {
        if self.cursor >= self.input.len() {
            return None;
        }

        let char = self.input[self.cursor..].chars().next()?;

        Some(self.cursor + char.len_utf8())
    }

    pub fn render(
        title: &str,
        input_state: &Input,
        placeholder: &str,
        frame: &mut Frame,
        area: Rect,
    ) {
        let is_placeholder = input_state.input.is_empty();

        let mut paragraph_border_style = Style::new().fg(theme().border);

        if input_state.is_focused {
            paragraph_border_style = Style::new().fg(theme().accent);
        }

        let mut paragraph_block = Block::default()
            .borders(Borders::ALL)
            .border_style(paragraph_border_style)
            .title(Span::styled(
                title.to_string(),
                Style::new().fg(theme().text),
            ));

        if input_state.is_focused {
            paragraph_block = paragraph_block.border_set(symbols::border::THICK);
        }

        let mut spans: Vec<Span> = vec![];

        if is_placeholder {
            spans.push(Span::styled(
                placeholder.to_owned(),
                Style::new().fg(theme().muted),
            ));
        } else if input_state.is_focused {
            let chars: Vec<char> = input_state.input.chars().collect();
            let cursor_index = input_state.chars_before_cursor();
            let before: String = chars[..cursor_index].iter().collect();
            let cursor_char = chars.get(cursor_index).copied();
            let after: String = if cursor_index < chars.len() {
                chars[cursor_index + 1..].iter().collect()
            } else {
                String::new()
            };
            let text_style = Style::new().fg(theme().text);
            let cursor_style = Style::new().fg(theme().accent_text).bg(theme().accent);

            spans.push(Span::styled(before, text_style));
            spans.push(Span::styled(
                cursor_char
                    .map(|char| char.to_string())
                    .unwrap_or_else(|| " ".to_string()),
                cursor_style,
            ));
            spans.push(Span::styled(after, text_style));
        } else {
            spans.push(Span::styled(
                input_state.input.to_owned(),
                Style::new().fg(theme().text),
            ));
        }

        let paragraph = Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .style(Style::new().bg(theme().background))
            .block(paragraph_block)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> Input {
        let mut input = Input::init();

        for char in text.chars() {
            input.insert_char(char);
        }

        input
    }

    #[test]
    fn typing_appends_and_moves_cursor() {
        let input = typed("самая классная за");

        assert_eq!(input.input, "самая классная за");
        assert_eq!(input.cursor, input.input.len());
    }

    #[test]
    fn backspace_keeps_the_space_before_the_removed_word() {
        let mut input = typed("самая классная за");

        input.backspace();
        input.backspace();

        assert_eq!(input.input, "самая классная ");
        assert_eq!(input.cursor, input.input.len());
    }

    #[test]
    fn backspace_on_empty_input_does_nothing() {
        let mut input = Input::init();

        input.backspace();

        assert_eq!(input.input, "");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn backspace_removes_whole_unicode_characters() {
        let mut input = typed("задача 😀");

        input.backspace();

        assert_eq!(input.input, "задача ");
    }

    #[test]
    fn typing_in_the_middle_inserts_at_cursor() {
        let mut input = typed("классная задача");

        input.move_cursor_home();
        input.insert_text("самая ");

        assert_eq!(input.input, "самая классная задача");
        assert_eq!(input.cursor, "самая ".len());
    }

    #[test]
    fn arrows_move_by_whole_characters() {
        let mut input = typed("абв");

        input.move_cursor_left();
        input.move_cursor_left();
        input.move_cursor_left();
        input.move_cursor_left();

        assert_eq!(input.cursor, 0);

        input.move_cursor_right();

        assert_eq!(input.cursor, "а".len());
    }

    #[test]
    fn home_and_end_jump_to_edges() {
        let mut input = typed("задача");

        input.move_cursor_home();
        assert_eq!(input.cursor, 0);

        input.move_cursor_end();
        assert_eq!(input.cursor, input.input.len());
    }

    #[test]
    fn delete_removes_character_under_cursor() {
        let mut input = typed("абв");

        input.move_cursor_home();
        input.delete_forward();

        assert_eq!(input.input, "бв");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn ctrl_backspace_removes_previous_word() {
        let mut input = typed("самая классная за");

        input.delete_word_backward();

        assert_eq!(input.input, "самая классная");
        assert_eq!(input.cursor, input.input.len());

        input.delete_word_backward();
        input.delete_word_backward();

        assert_eq!(input.input, "");
    }

    #[test]
    fn paste_strips_line_breaks_and_control_chars() {
        let mut input = typed("до ");

        input.insert_text("после\r\n\tвставки");

        assert_eq!(input.input, "до послевставки");
    }

    #[test]
    fn cursor_cell_is_highlighted_in_a_focused_field() {
        use ratatui::{backend::TestBackend, Terminal};

        let mut input = typed("абв");
        input.set_focus();
        input.move_cursor_home();
        input.move_cursor_right();

        let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();
        terminal
            .draw(|frame| Input::render("title", &input, "placeholder", frame, frame.size()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let cursor_cell = buffer
            .content
            .iter()
            .find(|cell| cell.symbol() == "б")
            .unwrap();
        let text_cell = buffer
            .content
            .iter()
            .find(|cell| cell.symbol() == "а")
            .unwrap();

        assert_ne!(cursor_cell.bg, text_cell.bg);
    }

    #[test]
    fn pasting_into_the_middle_keeps_the_tail() {
        let mut input = typed("начало конец");

        input.move_cursor_home();
        input.move_cursor_right();
        input.move_cursor_right();
        input.insert_text("XX");

        assert_eq!(input.input, "наXXчало конец");
    }
}
