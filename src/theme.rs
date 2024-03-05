use ratatui::prelude::*;

pub struct Theme {
    pub key_binding: KeyBinding,
}

pub struct KeyBinding {
    pub key: Style,
    pub description: Style,
}

pub const THEME: Theme = Theme {
    key_binding: KeyBinding {
        key: Style::new().fg(BLACK).bg(DARK_GRAY),
        description: Style::new().fg(DARK_GRAY).bg(BLACK),
    },
};

const BLACK: Color = Color::Rgb(8, 8, 8); // not really black, often #080808
const DARK_GRAY: Color = Color::Rgb(68, 68, 68);
