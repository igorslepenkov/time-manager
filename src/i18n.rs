//! Двуязычный интерфейс: английский по умолчанию, русский по выбору.
//!
//! Строки живут рядом с местом использования: `t("English", "Русский")`.
//! Для двух языков и одного приложения каталог из сотни ключей только мешает
//! сопровождать — а так текст нельзя забыть обновить рядом с кодом.

pub const CONFIG_DEFAULT: &str = "en";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
}

impl Lang {
    pub const fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
        }
    }

    pub fn from_code(code: &str) -> Lang {
        if code.trim().eq_ignore_ascii_case("ru") {
            Lang::Ru
        } else {
            Lang::En
        }
    }

    pub const fn next(self) -> Lang {
        match self {
            Lang::En => Lang::Ru,
            Lang::Ru => Lang::En,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Русский",
        }
    }
}

pub use crate::settings::{lang, set_lang};
/// Текст на текущем языке интерфейса.
pub fn t(english: &'static str, russian: &'static str) -> &'static str {
    t_for(lang(), english, russian)
}

/// Текст для конкретного языка — удобно проверять тестами без глобального состояния.
pub fn t_for(language: Lang, english: &'static str, russian: &'static str) -> &'static str {
    match language {
        Lang::En => english,
        Lang::Ru => russian,
    }
}

pub fn unit_hours(language: Lang) -> &'static str {
    t_for(language, "h", "ч")
}

pub fn unit_minutes(language: Lang) -> &'static str {
    t_for(language, "min", "мин")
}

/// Длительность словами: «3 ч 25 мин» / «3 h 25 min», «25 мин» без часов.
pub fn duration_text(seconds: i64) -> String {
    duration_text_for(lang(), seconds)
}

pub fn duration_text_for(language: Lang, seconds: i64) -> String {
    let seconds = seconds.max(0);
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;

    if hours > 0 {
        format!(
            "{hours}{} {minutes:02}{}",
            unit_hours(language),
            unit_minutes(language)
        )
    } else {
        format!("{minutes}{}", unit_minutes(language))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_language_codes() {
        assert_eq!(Lang::from_code("ru"), Lang::Ru);
        assert_eq!(Lang::from_code(" RU "), Lang::Ru);
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("de"), Lang::En);
    }

    #[test]
    fn translates_without_touching_global_state() {
        assert_eq!(t_for(Lang::En, "Home", "Главная"), "Home");
        assert_eq!(t_for(Lang::Ru, "Home", "Главная"), "Главная");
    }

    #[test]
    fn formats_durations_for_both_languages() {
        assert_eq!(duration_text_for(Lang::En, 3 * 3600 + 25 * 60), "3h 25min");
        assert_eq!(duration_text_for(Lang::Ru, 3 * 3600 + 25 * 60), "3ч 25мин");
        assert_eq!(duration_text_for(Lang::En, 25 * 60), "25min");
        assert_eq!(duration_text_for(Lang::Ru, 5 * 60), "5мин");
    }

    #[test]
    fn cycles_between_languages() {
        assert_eq!(Lang::En.next(), Lang::Ru);
        assert_eq!(Lang::Ru.next(), Lang::En);
        assert_eq!(Lang::Ru.code(), "ru");
        assert_eq!(Lang::En.name(), "English");
    }
}
