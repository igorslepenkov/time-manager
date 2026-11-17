//! Единое глобальное состояние интерфейса: выбранная тема и язык.
//!
//! Раньше это были две независимые ячейки (`theme` и `i18n`), теперь одна:
//! применение темы и переключение языка идут через одно место, и на всё
//! приложение один мьютекс вместо двух.

use std::sync::{Mutex, OnceLock};

use crate::i18n::Lang;
use crate::theme::{Theme, ThemeColors};

struct Settings {
    theme: Theme,
    lang: Lang,
}

static SETTINGS: OnceLock<Mutex<Settings>> = OnceLock::new();

fn settings_cell() -> &'static Mutex<Settings> {
    SETTINGS.get_or_init(|| {
        Mutex::new(Settings {
            theme: Theme::from_colors(&ThemeColors::default()),
            lang: Lang::En,
        })
    })
}

/// Применить цвета темы ко всему интерфейсу.
pub fn apply_colors(colors: &ThemeColors) {
    let mut settings = settings_cell().lock().unwrap();
    settings.theme = Theme::from_colors(colors);
}

pub fn set_lang(language: Lang) {
    let mut settings = settings_cell().lock().unwrap();
    settings.lang = language;
}

pub fn theme() -> Theme {
    settings_cell().lock().unwrap().theme.clone()
}

pub fn lang() -> Lang {
    settings_cell().lock().unwrap().lang
}
