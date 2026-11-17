use std::fs;
use std::path::Path;

use ratatui::style::Color as TuiColor;
use serde::{Deserialize, Serialize};

/// Срезает BOM, который появляется, если файл сохранили в Блокноте или
/// PowerShell 5.1: иначе JSON считается битым и данные теряются.
pub(crate) fn strip_bom(text: &str) -> &str {
    text.trim_start_matches('\u{feff}')
}

/// Настройки приложения. Лежат в config.json рядом с исполняемым файлом.
/// Если файла нет — он создаётся с настройками по умолчанию.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Язык интерфейса: "en" или "ru".
    pub language: String,
    pub theme: ThemeConfig,
    pub export: ExportConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            language: crate::i18n::CONFIG_DEFAULT.to_string(),
            theme: ThemeConfig::default(),
            export: ExportConfig::default(),
        }
    }
}

/// Результат чтения настроек: сами настройки и, если файл не разобрался,
/// текст ошибки для показа в интерфейсе.
pub struct LoadedConfig {
    pub config: Config,
    pub warning: Option<String>,
}

impl Config {
    pub fn load(path: &Path) -> LoadedConfig {
        if !path.exists() {
            let config = Config::default();
            let _ = config.save(path);

            return LoadedConfig {
                config,
                warning: None,
            };
        }

        match fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str(strip_bom(&text)) {
                Ok(config) => LoadedConfig {
                    config,
                    warning: None,
                },
                Err(error) => {
                    let backup_path = path.with_extension("json.broken");
                    let _ = fs::copy(path, &backup_path);

                    LoadedConfig {
                        config: Config::default(),
                        warning: Some(format!(
                            "{} ({}): {}. {}: {}",
                            crate::i18n::t("config.json is damaged", "config.json повреждён"),
                            path.display(),
                            error,
                            crate::i18n::t(
                                "defaults are used",
                                "используются настройки по умолчанию"
                            ),
                            backup_path.display()
                        )),
                    }
                }
            },
            Err(error) => LoadedConfig {
                config: Config::default(),
                warning: Some(format!(
                    "{} cannot be read ({}), default settings are used",
                    path.display(),
                    error
                )),
            },
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|error| {
            format!(
                "{}: {error}",
                crate::i18n::t(
                    "Could not prepare the settings",
                    "Не удалось подготовить настройки"
                )
            )
        })?;

        fs::write(path, text + "\n").map_err(|error| {
            format!(
                "{} {}: {error}",
                crate::i18n::t("Could not write", "Не удалось записать"),
                path.display()
            )
        })
    }
}

/// Выбранная тема интерфейса. Цвета живут в theme.json, здесь только имя,
/// чтобы можно было менять тему в два клика и не раздувать config.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub preset: String,
}
/// Описание выходной xlsx-таблицы: шрифт, цвета, высота строк, набор колонок.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportConfig {
    /// Формат даты, который идёт в имя листа и в имя файла (chrono strftime).
    pub sheet_name: String,
    /// Куда сохранять xlsx. Пусто — рядом с исполняемым файлом; поддерживаются
    /// %ПЕРЕМЕННАЯ% и начальный ~.
    pub output_dir: String,
    /// Строка с итогом по времени.
    pub totals_enabled: bool,
    pub totals_label: String,
    /// Открыть готовый файл в программе по умолчанию.
    pub open_after_export: bool,
    pub font_name: String,
    pub font_size: f64,
    pub text_color: String,
    pub background_color: String,
    pub header_enabled: bool,
    pub header_background_color: String,
    pub header_bold: bool,
    pub row_height: f64,
    pub columns: Vec<ColumnConfig>,
}

impl Default for ExportConfig {
    fn default() -> Self {
        ExportConfig {
            sheet_name: "%d.%m.%Y".to_string(),
            output_dir: String::new(),
            totals_enabled: true,
            totals_label: "Итого".to_string(),
            open_after_export: false,
            font_name: "Nunito".to_string(),
            font_size: 10.0,
            text_color: "#000000".to_string(),
            background_color: "#EEEEEE".to_string(),
            header_enabled: false,
            header_background_color: "#DDDDDD".to_string(),
            header_bold: true,
            row_height: 50.0,
            columns: default_columns(),
        }
    }
}

/// Колонка выходной таблицы. В формуле доступны подстановки:
/// {row} — номер строки, {start}, {end}, {duration} — буквы колонок.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnConfig {
    pub kind: ColumnKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        ColumnConfig {
            kind: ColumnKind::Empty,
            title: None,
            width: None,
            num_format: None,
            wrap: None,
            bold: None,
            formula: None,
        }
    }
}

impl ColumnConfig {
    pub fn title(&self) -> String {
        match &self.title {
            Some(title) => title.to_owned(),
            None => self.kind.default_title().to_string(),
        }
    }

    pub fn width(&self) -> f64 {
        self.width.unwrap_or_else(|| self.kind.default_width())
    }

    pub fn num_format(&self) -> String {
        match &self.num_format {
            Some(num_format) => num_format.to_owned(),
            None => self.kind.default_num_format().to_string(),
        }
    }

    pub fn wrap(&self) -> bool {
        self.wrap.unwrap_or_else(|| self.kind.default_wrap())
    }

    pub fn bold(&self) -> bool {
        self.bold.unwrap_or_else(|| self.kind.default_bold())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    Date,
    Tag,
    Task,
    Start,
    End,
    Duration,
    Hours,
    Empty,
}

impl ColumnKind {
    pub fn default_title(&self) -> &'static str {
        match self {
            ColumnKind::Date => "Дата",
            ColumnKind::Tag => "Тег",
            ColumnKind::Task => "Задача",
            ColumnKind::Start => "Начало",
            ColumnKind::End => "Конец",
            ColumnKind::Duration => "Длительность",
            ColumnKind::Hours => "Часы",
            ColumnKind::Empty => "",
        }
    }

    fn default_width(&self) -> f64 {
        match self {
            ColumnKind::Date => 12.0,
            ColumnKind::Tag => 25.0,
            ColumnKind::Task => 40.0,
            ColumnKind::Start | ColumnKind::End => 10.0,
            ColumnKind::Duration => 12.0,
            ColumnKind::Hours => 10.0,
            ColumnKind::Empty => 3.0,
        }
    }

    fn default_num_format(&self) -> &'static str {
        match self {
            ColumnKind::Date => "dd.mm.yyyy",
            ColumnKind::Start | ColumnKind::End | ColumnKind::Duration => "hh:mm",
            ColumnKind::Hours => "0.00",
            _ => "",
        }
    }

    fn default_wrap(&self) -> bool {
        matches!(self, ColumnKind::Task | ColumnKind::Tag)
    }

    fn default_bold(&self) -> bool {
        matches!(self, ColumnKind::Hours)
    }
}

fn default_columns() -> Vec<ColumnConfig> {
    vec![
        ColumnConfig {
            kind: ColumnKind::Date,
            title: Some("Дата".to_string()),
            width: Some(12.0),
            num_format: Some("dd.mm.yyyy".to_string()),
            ..Default::default()
        },
        ColumnConfig {
            kind: ColumnKind::Tag,
            title: Some("Тег".to_string()),
            width: Some(25.0),
            ..Default::default()
        },
        ColumnConfig {
            kind: ColumnKind::Task,
            title: Some("Задача".to_string()),
            width: Some(40.0),
            wrap: Some(true),
            ..Default::default()
        },
        ColumnConfig {
            kind: ColumnKind::Start,
            title: Some("Начало".to_string()),
            width: Some(10.0),
            num_format: Some("hh:mm".to_string()),
            ..Default::default()
        },
        ColumnConfig {
            kind: ColumnKind::End,
            title: Some("Конец".to_string()),
            width: Some(10.0),
            num_format: Some("hh:mm".to_string()),
            ..Default::default()
        },
        ColumnConfig {
            kind: ColumnKind::Duration,
            title: Some("Длительность".to_string()),
            width: Some(12.0),
            num_format: Some("hh:mm".to_string()),
            formula: Some("={end}{row}-{start}{row}".to_string()),
            ..Default::default()
        },
        ColumnConfig {
            kind: ColumnKind::Hours,
            title: Some("Часы".to_string()),
            width: Some(10.0),
            num_format: Some("0.00".to_string()),
            bold: Some(true),
            formula: Some(
                "=ROUND(HOUR({duration}{row})+MINUTE({duration}{row})/60+SECOND({duration}{row})/3600, 2)"
                    .to_string(),
            ),
            ..Default::default()
        },
    ]
}

/// Приводит цвет к виду #RRGGBB, который понимает rust_xlsxwriter.
pub fn normalize_hex_color(value: &str, fallback: &str) -> String {
    let normalized = value.trim().trim_start_matches('#');

    if normalized.len() == 6 && normalized.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("#{}", normalized.to_ascii_uppercase())
    } else {
        normalize_hex_color(fallback, "#000000")
    }
}

pub(crate) fn parse_color(value: &str, fallback: TuiColor) -> TuiColor {
    let normalized = value.trim().trim_start_matches('#');

    if normalized.len() == 6 {
        if let Ok(rgb) = u32::from_str_radix(normalized, 16) {
            return TuiColor::Rgb(
                ((rgb >> 16) & 0xFF) as u8,
                ((rgb >> 8) & 0xFF) as u8,
                (rgb & 0xFF) as u8,
            );
        }
    }

    match value.trim().to_ascii_lowercase().as_str() {
        "black" => TuiColor::Black,
        "white" => TuiColor::White,
        "red" => TuiColor::Red,
        "green" => TuiColor::Green,
        "yellow" => TuiColor::Yellow,
        "blue" => TuiColor::Blue,
        "magenta" => TuiColor::Magenta,
        "cyan" => TuiColor::Cyan,
        "gray" | "grey" => TuiColor::Gray,
        "dark_gray" | "darkgray" | "dark_grey" => TuiColor::DarkGray,
        "reset" | "default" => TuiColor::Reset,
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_named_colors() {
        assert_eq!(
            parse_color("#FF0000", TuiColor::Black),
            TuiColor::Rgb(255, 0, 0)
        );
        assert_eq!(
            parse_color("00FF00", TuiColor::Black),
            TuiColor::Rgb(0, 255, 0)
        );
        assert_eq!(parse_color("blue", TuiColor::Black), TuiColor::Blue);
        assert_eq!(parse_color("nonsense", TuiColor::Red), TuiColor::Red);
    }

    #[test]
    fn normalizes_excel_colors() {
        assert_eq!(normalize_hex_color("#eeeeee", "#FFFFFF"), "#EEEEEE");
        assert_eq!(normalize_hex_color("aabbcc", "#FFFFFF"), "#AABBCC");
        assert_eq!(normalize_hex_color("bad", "#FFFFFF"), "#FFFFFF");
    }

    #[test]
    fn default_config_round_trips_through_json() {
        let config = Config::default();
        let text = serde_json::to_string_pretty(&config).unwrap();
        let parsed: Config = serde_json::from_str(&text).unwrap();

        assert_eq!(parsed.export.columns.len(), 7);
        assert_eq!(parsed.export.columns[0].kind, ColumnKind::Date);
        assert_eq!(parsed.export.columns[6].kind, ColumnKind::Hours);
    }

    #[test]
    fn partial_config_uses_defaults() {
        let parsed: Config = serde_json::from_str(
            r##"{"export": {"font_name": "Arial", "columns": [{"kind": "task"}]}}"##,
        )
        .unwrap();

        assert_eq!(parsed.export.font_name, "Arial");
        assert_eq!(parsed.export.font_size, 10.0);
        assert_eq!(parsed.export.columns.len(), 1);
        assert_eq!(parsed.export.columns[0].num_format(), "");
    }

    #[test]
    fn saves_and_restores_language() {
        let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output")
            .join("config-language");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("config.json");

        let config = Config {
            language: "ru".to_string(),
            ..Config::default()
        };
        config.save(&path).unwrap();

        let loaded = Config::load(&path);
        let text = std::fs::read_to_string(&path).unwrap();

        assert_eq!(loaded.config.language, "ru");
        assert!(text.contains("\"language\": \"ru\""));
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn writes_default_config_when_missing() {
        let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output")
            .join("config");
        std::fs::create_dir_all(&directory).unwrap();

        let path = directory.join("config.json");
        let _ = std::fs::remove_file(&path);

        let loaded = Config::load(&path);

        assert!(loaded.warning.is_none());
        assert!(path.exists());

        let parsed: Config =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed.export.columns.len(), 7);
        assert_eq!(parsed.theme.preset, "");
    }

    #[test]
    fn reports_broken_config() {
        let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output")
            .join("config");
        std::fs::create_dir_all(&directory).unwrap();

        let path = directory.join("broken.json");
        let _ = std::fs::remove_file(path.with_extension("json.broken"));
        std::fs::write(&path, "{ not json }").unwrap();

        let loaded = Config::load(&path);

        assert!(loaded.warning.is_some());
        assert_eq!(loaded.config.export.font_name, "Nunito");
        assert!(path.with_extension("json.broken").exists());
    }
}
