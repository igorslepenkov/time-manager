use itertools::Itertools;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    symbols,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

#[derive(Clone)]
pub struct Input {
    pub is_focused: bool,
    pub input: String,
}

impl Input {
    pub fn init() -> Input {
        Input {
            is_focused: false,
            input: String::new(),
        }
    }

    pub fn clear_input(&mut self) -> () {
        self.input = String::new();
        self.is_focused = false;
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

    pub fn render(
        title: &str,
        input_state: &Input,
        placeholder: &str,
        frame: &mut Frame,
        area: Rect,
    ) {
        let input = if input_state.input.len() > 0 {
            input_state.input.to_owned()
        } else {
            placeholder.to_owned()
        };

        let mut paragraph_border_style = Style::new().blue();

        if input_state.is_focused {
            paragraph_border_style = paragraph_border_style.yellow();
        }

        let mut paragraph_block = Block::default()
            .borders(Borders::ALL)
            .border_style(paragraph_border_style)
            .title(title);

        if input_state.is_focused {
            paragraph_block = paragraph_block.border_set(symbols::border::THICK);
        }

        let paragraph = Paragraph::new(input)
            .alignment(Alignment::Center)
            .block(paragraph_block)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    pub fn add_char_to_input(&mut self, char: char) {
        self.input = self.input.to_owned() + &char.to_string();
    }

    pub fn remove_last_char_from_input(&mut self) {
        let input_string = self.input.to_owned();

        if input_string.len() == 0 {
            return;
        }

        if input_string.len() == 1 {
            self.input = String::new();

            return;
        }

        let vector: Vec<&str> = input_string.split("").collect();

        // Неодиданно, но split возвращает пустые строки для начала и конца строки
        // поэтому использую -2 и затем trim чтобы обойти эту проблему
        let slice = vector.iter().take(vector.len() - 2).join("");

        self.input = slice.trim().to_string();
    }
}
