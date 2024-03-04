use ratatui::{
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    symbols,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::utils;

#[derive(Clone)]
pub struct SubmitButton {
    pub is_focused: bool,
}

impl SubmitButton {
    pub fn init() -> SubmitButton {
        SubmitButton { is_focused: false }
    }

    pub fn toggle_focus(&mut self) {
        self.is_focused = !self.is_focused
    }

    pub fn set_focus(&mut self) {
        self.is_focused = true
    }

    pub fn unset_focus(&mut self) {
        self.is_focused = false
    }

    pub fn render(state: &SubmitButton, frame: &mut Frame, area: Rect) {
        let centered_button = utils::centered_rect(area, 50, 100);

        let mut paragraph_border_style = Style::new().blue();

        if state.is_focused {
            paragraph_border_style = paragraph_border_style.yellow();
        }

        let mut paragraph_block = Block::default()
            .borders(Borders::ALL)
            .border_style(paragraph_border_style);

        let mut paragraph_style = Style::new().on_blue();

        if state.is_focused {
            paragraph_block = paragraph_block.border_set(symbols::border::THICK);
            paragraph_style = paragraph_style.on_yellow();
        }

        let paragraph = Paragraph::new("Submit")
            .alignment(Alignment::Center)
            .block(paragraph_block)
            .style(paragraph_style);

        frame.render_widget(paragraph, centered_button);
    }
}
