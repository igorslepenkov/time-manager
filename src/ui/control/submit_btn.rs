use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    symbols,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::theme::theme;
use crate::utils;

#[derive(Clone, PartialEq)]
pub struct SubmitButton {
    pub is_focused: bool,
}

impl SubmitButton {
    pub fn init() -> SubmitButton {
        SubmitButton { is_focused: false }
    }

    pub fn set_focus(&mut self) {
        self.is_focused = true
    }

    pub fn unset_focus(&mut self) {
        self.is_focused = false
    }

    pub fn render(state: &SubmitButton, frame: &mut Frame, area: Rect, label: &str) {
        let centered_button = utils::centered_rect(area, 50, 100);

        let mut paragraph_border_style = Style::new().fg(theme().border);

        if state.is_focused {
            paragraph_border_style = Style::new().fg(theme().accent);
        }

        let mut paragraph_block = Block::default()
            .borders(Borders::ALL)
            .border_style(paragraph_border_style);

        let mut paragraph_style = Style::new().fg(theme().panel_text).bg(theme().panel);

        if state.is_focused {
            paragraph_block = paragraph_block.border_set(symbols::border::THICK);
            paragraph_style = Style::new().fg(theme().accent_text).bg(theme().accent);
        }

        let paragraph = Paragraph::new(label)
            .alignment(Alignment::Center)
            .block(paragraph_block)
            .style(paragraph_style);

        frame.render_widget(paragraph, centered_button);
    }
}
