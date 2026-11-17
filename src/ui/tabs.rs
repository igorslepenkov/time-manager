use crate::i18n::t;

#[derive(Debug, Eq, PartialEq)]
pub enum Tab {
    Home,
    Start,
    Out,
    End,
    ClearState,
    Theme,
}

impl Tab {
    /// Сколько вкладок в панели.
    pub const COUNT: usize = 6;

    /// Позиция вкладки в панели — не зависит от языка.
    pub const fn index(&self) -> usize {
        match self {
            Tab::Home => 0,
            Tab::Start => 1,
            Tab::Out => 2,
            Tab::End => 3,
            Tab::ClearState => 4,
            Tab::Theme => 5,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Tab::Home => t("Home", "Главная"),
            Tab::Start => t("Start", "Старт"),
            Tab::Out => t("Pause", "Пауза"),
            Tab::End => t("End of day", "Конец дня"),
            Tab::Theme => t("Config", "Конфиг"),
            Tab::ClearState => t("Clear day", "Сброс дня"),
        }
    }

    pub fn as_string_vec() -> Vec<String> {
        [
            Tab::Home,
            Tab::Start,
            Tab::Out,
            Tab::End,
            Tab::ClearState,
            Tab::Theme,
        ]
        .iter()
        .map(|tab| tab.label().to_string())
        .collect()
    }
}
