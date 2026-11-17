use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::prelude::*;
use serde::{Deserialize, Serialize};

use crate::config::{parse_color, strip_bom};
use crate::i18n::t;

/// Старое имя файла тем: нужно только для миграции в папку `themes`.
pub const LEGACY_THEME_FILE: &str = "theme.json";
pub const THEMES_DIRECTORY: &str = "themes";
/// Сколько проблемных файлов перечислять в предупреждении, остальные — числом.
const MAX_LISTED_PROBLEMS: usize = 3;
const COLOR_ROLES: [&str; 9] = [
    "background",
    "text",
    "muted",
    "panel",
    "panel_text",
    "accent",
    "accent_text",
    "border",
    "error",
];

/// Роли цветов интерфейса. Одна тема задаёт цвета для всех ролей сразу.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeColors {
    /// Фон окна.
    pub background: String,
    /// Обычный текст.
    pub text: String,
    /// Приглушённый текст: подсказки, плейсхолдеры.
    pub muted: String,
    /// Фон верхних плашек.
    pub panel: String,
    /// Текст на плашках.
    pub panel_text: String,
    /// Активный элемент: вкладка, кнопка в фокусе, подсветка строки.
    pub accent: String,
    /// Текст на акценте.
    pub accent_text: String,
    /// Рамки блоков и полей ввода.
    pub border: String,
    /// Предупреждения и ошибки.
    pub error: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        ThemeColors {
            background: "#080808".to_string(),
            text: "#FFFFFF".to_string(),
            muted: "#787878".to_string(),
            panel: "#0000AF".to_string(),
            panel_text: "#FFFFFF".to_string(),
            accent: "#FFFF00".to_string(),
            accent_text: "#080808".to_string(),
            border: "#3C3C3C".to_string(),
            error: "#FF5F5F".to_string(),
        }
    }
}

/// Готовая тема: имя и цвета всех ролей. Имя необязательно — если его нет,
/// берётся имя файла.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemePreset {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub colors: ThemeColors,
}

/// Старый формат: массив тем в одном `theme.json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ThemeFile {
    #[serde(default)]
    pub themes: Vec<ThemePreset>,
}

/// Откуда взялась тема: из бинарника или из файла в `themes/`.
#[derive(Clone, Debug)]
pub enum ThemeSource {
    Builtin,
    File(PathBuf),
}

pub struct LoadedThemes {
    pub presets: Vec<ThemePreset>,
    pub sources: Vec<ThemeSource>,
    pub warning: Option<String>,
}

/// Собирает темы: встроенные из бинарника плюс файлы из `<рядом с exe>/themes`.
/// Файл с именем встроенной темы пропускается — встроенная всегда главнее.
pub fn load_themes(directory: &Path) -> LoadedThemes {
    let mut problems: Vec<String> = vec![];

    if let Some(migration) = migrate_legacy_theme_file(directory) {
        problems.push(migration);
    }

    let mut presets = builtin_presets();
    let mut sources = vec![ThemeSource::Builtin; presets.len()];
    load_custom_themes(directory, &mut presets, &mut sources, &mut problems);

    LoadedThemes {
        presets,
        sources,
        warning: summarize(problems),
    }
}

/// Переводит старый `theme.json` в папку `themes/`.
///
/// Миграция идемпотентна: триггер — наличие `theme.json`, файлы пишутся
/// «пропустить, если уже есть», а сам `theme.json` переименовывается в
/// `theme.json.old` только после того, как все файлы на месте.
fn migrate_legacy_theme_file(directory: &Path) -> Option<String> {
    let legacy_path = directory.join(LEGACY_THEME_FILE);

    if !legacy_path.exists() {
        return None;
    }

    let text = match fs::read_to_string(&legacy_path) {
        Ok(text) => text,
        Err(error) => {
            return Some(format!(
                "{} {}: {error}",
                t("Cannot read", "Не удалось прочитать"),
                legacy_path.display()
            ));
        }
    };

    let theme_file: ThemeFile = match serde_json::from_str(strip_bom(&text)) {
        Ok(theme_file) => theme_file,
        Err(error) => {
            return Some(format!(
                "{} {}: {error}. {}",
                t("theme.json is damaged", "theme.json повреждён"),
                legacy_path.display(),
                t("the file is left untouched", "файл оставлен без изменений")
            ));
        }
    };

    let themes_directory = directory.join(THEMES_DIRECTORY);

    if themes_directory.exists() && !themes_directory.is_dir() {
        return Some(format!(
            "{} {} {}",
            t("Path", "Путь"),
            themes_directory.display(),
            t(
                "is not a folder, themes were not migrated",
                "не папка, темы не перенесены"
            )
        ));
    }

    if let Err(error) = fs::create_dir_all(&themes_directory) {
        return Some(format!(
            "{} {}: {error}. {}",
            t("Could not create", "Не удалось создать"),
            themes_directory.display(),
            t("theme.json is kept", "theme.json сохранён")
        ));
    }

    let builtins = builtin_presets();
    let mut used_paths: HashSet<PathBuf> = HashSet::new();

    for preset in &theme_file.themes {
        if builtins.iter().any(|builtin| same_theme(builtin, preset)) {
            // Точная копия встроенной темы: в коде она уже есть.
            continue;
        }

        let mut name = preset.name.trim().to_string();

        if name.is_empty() {
            name = "Theme".to_string();
        }

        if builtins
            .iter()
            .any(|builtin| builtin.name.to_lowercase() == name.to_lowercase())
        {
            // Встроенная главнее, поэтому правленую копию переносим под новым именем.
            name = format!("{name} (custom)");
        }

        let path = next_free_path(&themes_directory, &name, &mut used_paths);

        if path.exists() {
            continue;
        }

        let theme = ThemePreset {
            name: name.clone(),
            colors: preset.colors.clone(),
        };
        let content = match serde_json::to_string_pretty(&theme) {
            Ok(content) => content + "\n",
            Err(error) => {
                return Some(format!(
                    "{}: {error}. {}",
                    t(
                        "Could not serialize the theme",
                        "Не удалось сериализовать тему"
                    ),
                    t("theme.json is kept", "theme.json сохранён")
                ));
            }
        };

        if let Err(error) = fs::write(&path, content) {
            return Some(format!(
                "{} {}: {error}. {}",
                t("Could not write", "Не удалось записать"),
                path.display(),
                t("theme.json is kept", "theme.json сохранён")
            ));
        }
    }

    let archived_path = legacy_path.with_extension("json.old");
    let _ = fs::remove_file(&archived_path);

    if let Err(error) = fs::rename(&legacy_path, &archived_path) {
        return Some(format!(
            "{} {}: {error}",
            t(
                "Could not rename theme.json",
                "Не удалось переименовать theme.json"
            ),
            legacy_path.display()
        ));
    }

    None
}

fn load_custom_themes(
    directory: &Path,
    presets: &mut Vec<ThemePreset>,
    sources: &mut Vec<ThemeSource>,
    problems: &mut Vec<String>,
) {
    let themes_directory = directory.join(THEMES_DIRECTORY);

    if !themes_directory.exists() {
        return;
    }

    if !themes_directory.is_dir() {
        problems.push(format!(
            "{} {}",
            t("Path", "Путь"),
            t(
                "themes is not a folder, custom themes are ignored",
                "themes не папка, свои темы не загружены"
            )
        ));

        return;
    }

    let entries = match fs::read_dir(&themes_directory) {
        Ok(entries) => entries,
        Err(error) => {
            problems.push(format!(
                "{} {}: {error}",
                t("Could not read", "Не удалось прочитать"),
                themes_directory.display()
            ));

            return;
        }
    };

    let mut files = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .map(|extension| extension.to_string_lossy().eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
        })
        .collect::<Vec<PathBuf>>();

    files.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });

    for path in files {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                problems.push(format!("{}: {error}", path.display()));

                continue;
            }
        };

        let theme: ThemePreset = match serde_json::from_str(strip_bom(&text)) {
            Ok(theme) => theme,
            Err(error) => {
                problems.push(format!("{}: {error}", path.display()));

                continue;
            }
        };

        let fallback_name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut name = theme.name.trim().to_string();

        if name.is_empty() {
            name = fallback_name;
        }

        if presets
            .iter()
            .any(|preset| preset.name.to_lowercase() == name.to_lowercase())
        {
            problems.push(format!(
                "{}: {} «{}» {}",
                path.display(),
                t("theme", "тема"),
                name,
                t(
                    "already exists, the file is skipped",
                    "уже есть, файл пропущен"
                )
            ));

            continue;
        }

        if let Some(missing) = missing_roles(&text) {
            problems.push(format!(
                "{}: {} {}",
                path.display(),
                t("missing roles", "не хватает ролей"),
                missing
            ));
        }

        sources.push(ThemeSource::File(path));
        presets.push(ThemePreset {
            name,
            colors: theme.colors,
        });
    }
}

fn missing_roles(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(strip_bom(text)).ok()?;
    let colors = value.get("colors");
    let missing = COLOR_ROLES
        .iter()
        .filter(|role| colors.and_then(|colors| colors.get(**role)).is_none())
        .copied()
        .collect::<Vec<&str>>();

    if missing.is_empty() {
        None
    } else {
        Some(missing.join(", "))
    }
}

fn same_theme(left: &ThemePreset, right: &ThemePreset) -> bool {
    left.name.to_lowercase() == right.name.to_lowercase() && left.colors == right.colors
}

/// Путь для темы: имя файла чистится от недопустимых символов и конфликтов.
fn next_free_path(directory: &Path, name: &str, used: &mut HashSet<PathBuf>) -> PathBuf {
    let base = sanitize_theme_file_name(name);
    let mut candidate = directory.join(format!("{base}.json"));
    let mut index = 2;

    while used.contains(&candidate) {
        candidate = directory.join(format!("{base}-{index}.json"));
        index += 1;
    }

    used.insert(candidate.clone());

    candidate
}

fn sanitize_theme_file_name(name: &str) -> String {
    let cleaned = name
        .trim()
        .chars()
        .map(|char| match char {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            char if char.is_control() => '-',
            char => char,
        })
        .take(60)
        .collect::<String>();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    let mut cleaned = if cleaned.is_empty() {
        "theme".to_string()
    } else {
        cleaned
    };

    let reserved = matches!(
        cleaned.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );

    if reserved {
        cleaned = format!("_{cleaned}");
    }

    cleaned
}

fn summarize(problems: Vec<String>) -> Option<String> {
    if problems.is_empty() {
        return None;
    }

    let total = problems.len();
    let mut text = problems
        .into_iter()
        .take(MAX_LISTED_PROBLEMS)
        .collect::<Vec<String>>()
        .join("; ");

    if total > MAX_LISTED_PROBLEMS {
        text.push_str(&format!(
            "; {} {}",
            t("and more:", "и ещё:"),
            total - MAX_LISTED_PROBLEMS
        ));
    }

    Some(text)
}

/// Номер темы по имени без учёта регистра (в том числе для кириллицы).
pub fn find_preset_index(presets: &[ThemePreset], name: &str) -> Option<usize> {
    let wanted = name.trim().to_lowercase();

    presets
        .iter()
        .position(|preset| preset.name.to_lowercase() == wanted)
}

/// Цвета интерфейса в виде, удобном для отрисовки.
#[derive(Clone)]
pub struct Theme {
    pub background: Color,
    pub text: Color,
    pub muted: Color,
    pub panel: Color,
    pub panel_text: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub border: Color,
    pub error: Color,
    pub key_binding: KeyBinding,
}

#[derive(Clone)]
pub struct KeyBinding {
    pub key: Style,
    pub description: Style,
}

impl Theme {
    pub fn from_colors(colors: &ThemeColors) -> Theme {
        let background = parse_color(&colors.background, Color::Black);
        let muted = parse_color(&colors.muted, Color::DarkGray);

        Theme {
            background,
            text: parse_color(&colors.text, Color::White),
            muted,
            panel: parse_color(&colors.panel, Color::Blue),
            panel_text: parse_color(&colors.panel_text, Color::White),
            accent: parse_color(&colors.accent, Color::Yellow),
            accent_text: parse_color(&colors.accent_text, Color::Black),
            border: parse_color(&colors.border, Color::DarkGray),
            error: parse_color(&colors.error, Color::Red),
            key_binding: KeyBinding {
                key: Style::new()
                    .fg(parse_color(&colors.accent_text, Color::Black))
                    .bg(parse_color(&colors.accent, Color::Yellow)),
                description: Style::new().fg(muted).bg(background),
            },
        }
    }
}

pub use crate::settings::{apply_colors, theme};
/// Цвет подписи поверх заливки, чтобы читалось и на светлом, и на тёмном.
pub fn contrast_text(color: Color) -> Color {
    match color {
        Color::Rgb(red, green, blue) => {
            let luminance = 0.299 * red as f32 + 0.587 * green as f32 + 0.114 * blue as f32;

            if luminance > 140.0 {
                Color::Black
            } else {
                Color::White
            }
        }
        _ => Color::White,
    }
}

#[allow(clippy::too_many_arguments)]
fn preset(
    name: &str,
    background: &str,
    text: &str,
    muted: &str,
    panel: &str,
    panel_text: &str,
    accent: &str,
    accent_text: &str,
    border: &str,
    error: &str,
) -> ThemePreset {
    ThemePreset {
        name: name.to_string(),
        colors: ThemeColors {
            background: background.to_string(),
            text: text.to_string(),
            muted: muted.to_string(),
            panel: panel.to_string(),
            panel_text: panel_text.to_string(),
            accent: accent.to_string(),
            accent_text: accent_text.to_string(),
            border: border.to_string(),
            error: error.to_string(),
        },
    }
}

/// Встроенные темы. Цвета взяты из официальных палитр схем:
/// Dracula, Nord, Gruvbox, Solarized, Catppuccin, Tokyo Night,
/// One Dark, Monokai, GitHub, Everforest, Ayu.
pub fn builtin_presets() -> Vec<ThemePreset> {
    vec![
        preset(
            "Default", "#080808", "#FFFFFF", "#787878", "#0000AF", "#FFFFFF", "#FFFF00", "#080808",
            "#3C3C3C", "#FF5F5F",
        ),
        preset(
            "Dracula", "#282A36", "#F8F8F2", "#6272A4", "#44475A", "#F8F8F2", "#BD93F9", "#282A36",
            "#6272A4", "#FF5555",
        ),
        preset(
            "Alucard Light",
            "#FFFBEB",
            "#1F1F1F",
            "#6C664B",
            "#EFEDDC",
            "#1F1F1F",
            "#644AC9",
            "#FFFBEB",
            "#CFCFDE",
            "#CB3A2A",
        ),
        preset(
            "Nord", "#2E3440", "#ECEFF4", "#4C566A", "#3B4252", "#ECEFF4", "#88C0D0", "#2E3440",
            "#434C5E", "#BF616A",
        ),
        preset(
            "Gruvbox Dark",
            "#282828",
            "#EBDBB2",
            "#928374",
            "#3C3836",
            "#EBDBB2",
            "#FABD2F",
            "#282828",
            "#504945",
            "#FB4934",
        ),
        preset(
            "Gruvbox Light",
            "#FBF1C7",
            "#3C3836",
            "#928374",
            "#EBDBB2",
            "#3C3836",
            "#B57614",
            "#FBF1C7",
            "#D5C4A1",
            "#9D0006",
        ),
        preset(
            "Solarized Dark",
            "#002B36",
            "#839496",
            "#586E75",
            "#073642",
            "#93A1A1",
            "#268BD2",
            "#002B36",
            "#586E75",
            "#DC322F",
        ),
        preset(
            "Solarized Light",
            "#FDF6E3",
            "#657B83",
            "#93A1A1",
            "#EEE8D5",
            "#586E75",
            "#268BD2",
            "#FDF6E3",
            "#93A1A1",
            "#DC322F",
        ),
        preset(
            "One Dark", "#282C34", "#ABB2BF", "#5C6370", "#3E4451", "#ABB2BF", "#61AFEF",
            "#282C34", "#3E4451", "#E06C75",
        ),
        preset(
            "Monokai", "#272822", "#F8F8F2", "#75715E", "#3E3D32", "#F8F8F2", "#A6E22E", "#272822",
            "#49483E", "#F92672",
        ),
        preset(
            "Tokyo Night",
            "#1A1B26",
            "#C0CAF5",
            "#565F89",
            "#24283B",
            "#C0CAF5",
            "#7AA2F7",
            "#1A1B26",
            "#414868",
            "#F7768E",
        ),
        preset(
            "Catppuccin Mocha",
            "#1E1E2E",
            "#CDD6F4",
            "#6C7086",
            "#313244",
            "#CDD6F4",
            "#CBA6F7",
            "#1E1E2E",
            "#45475A",
            "#F38BA8",
        ),
        preset(
            "GitHub Dark",
            "#0D1117",
            "#C9D1D9",
            "#8B949E",
            "#161B22",
            "#C9D1D9",
            "#58A6FF",
            "#0D1117",
            "#30363D",
            "#F85149",
        ),
        preset(
            "GitHub Light",
            "#FFFFFF",
            "#1F2328",
            "#6E7781",
            "#F6F8FA",
            "#1F2328",
            "#0969DA",
            "#FFFFFF",
            "#D0D7DE",
            "#CF222E",
        ),
        preset(
            "Everforest",
            "#2D353B",
            "#D3C6AA",
            "#859289",
            "#343F44",
            "#D3C6AA",
            "#A7C080",
            "#2D353B",
            "#475258",
            "#E67E80",
        ),
        preset(
            "Ayu Dark", "#0B0E14", "#BFBDB6", "#565B66", "#131721", "#BFBDB6", "#E6B673",
            "#0B0E14", "#1F2430", "#F07178",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_directory(name: &str) -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output")
            .join("themes")
            .join(name);
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    fn custom_preset(name: &str) -> ThemePreset {
        ThemePreset {
            name: name.to_string(),
            colors: ThemeColors {
                background: "#101010".to_string(),
                accent: "#FF00AA".to_string(),
                ..ThemeColors::default()
            },
        }
    }

    fn write_theme(path: &Path, preset: &ThemePreset) {
        fs::write(path, serde_json::to_string_pretty(preset).unwrap()).unwrap();
    }

    #[test]
    fn builtin_themes_have_unique_names() {
        let presets = builtin_presets();
        let mut names = presets
            .iter()
            .map(|preset| preset.name.to_lowercase())
            .collect::<Vec<String>>();

        names.sort();
        names.dedup();

        assert_eq!(names.len(), presets.len());
    }

    #[test]
    fn builtin_themes_have_parseable_colors() {
        for preset in builtin_presets() {
            let theme = Theme::from_colors(&preset.colors);

            assert!(matches!(theme.background, Color::Rgb(..)));
        }
    }

    #[test]
    fn finds_theme_by_name_ignoring_case() {
        let presets = builtin_presets();
        let index = find_preset_index(&presets, "dRaCuLa").unwrap();

        assert_eq!(presets[index].name, "Dracula");
    }

    #[test]
    fn finds_theme_with_cyrillic_name() {
        let presets = vec![custom_preset("Моя тема")];

        assert!(find_preset_index(&presets, "моя ТЕМА").is_some());
    }

    #[test]
    fn missing_folder_keeps_builtins_without_warning() {
        let directory = test_directory("missing");
        let loaded = load_themes(&directory);

        assert_eq!(loaded.presets.len(), builtin_presets().len());
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn loads_theme_from_folder() {
        let directory = test_directory("loads");
        fs::create_dir_all(directory.join(THEMES_DIRECTORY)).unwrap();
        write_theme(
            &directory.join(THEMES_DIRECTORY).join("custom.json"),
            &custom_preset("Моя тема"),
        );

        let loaded = load_themes(&directory);

        assert_eq!(loaded.presets.len(), builtin_presets().len() + 1);
        assert!(loaded.warning.is_none());
        assert_eq!(loaded.presets.last().unwrap().name, "Моя тема");
    }

    #[test]
    fn theme_without_name_uses_file_stem() {
        let directory = test_directory("no-name");
        let themes_directory = directory.join(THEMES_DIRECTORY);
        fs::create_dir_all(&themes_directory).unwrap();
        fs::write(
            themes_directory.join("ocean.json"),
            r##"{"colors":{"accent":"#00AAFF"}}"##,
        )
        .unwrap();

        let loaded = load_themes(&directory);
        let custom = loaded.presets.last().unwrap();

        assert_eq!(custom.name, "ocean");
        assert!(loaded.warning.unwrap().contains("missing roles"));
    }

    #[test]
    fn file_with_builtin_name_is_skipped() {
        let directory = test_directory("shadow");
        let themes_directory = directory.join(THEMES_DIRECTORY);
        fs::create_dir_all(&themes_directory).unwrap();
        write_theme(
            &themes_directory.join("mine.json"),
            &ThemePreset {
                name: "dracula".to_string(),
                colors: ThemeColors {
                    accent: "#000000".to_string(),
                    ..ThemeColors::default()
                },
            },
        );

        let loaded = load_themes(&directory);
        let dracula = find_preset_index(&loaded.presets, "Dracula").unwrap();

        assert_eq!(loaded.presets.len(), builtin_presets().len());
        assert_eq!(loaded.presets[dracula].colors.accent, "#BD93F9");
        assert!(loaded.warning.unwrap().contains("already exists"));
    }

    #[test]
    fn duplicate_custom_names_keep_the_first_file() {
        let directory = test_directory("duplicates");
        let themes_directory = directory.join(THEMES_DIRECTORY);
        fs::create_dir_all(&themes_directory).unwrap();
        write_theme(&themes_directory.join("a.json"), &custom_preset("Mine"));
        write_theme(&themes_directory.join("b.json"), &custom_preset("Mine"));

        let loaded = load_themes(&directory);
        let custom_count = loaded
            .presets
            .iter()
            .filter(|preset| preset.name == "Mine")
            .count();

        assert_eq!(custom_count, 1);
        assert!(loaded.warning.unwrap().contains("already exists"));
    }

    #[test]
    fn broken_file_is_reported_but_others_load() {
        let directory = test_directory("broken");
        let themes_directory = directory.join(THEMES_DIRECTORY);
        fs::create_dir_all(&themes_directory).unwrap();
        fs::write(themes_directory.join("broken.json"), "{ not json").unwrap();
        write_theme(&themes_directory.join("good.json"), &custom_preset("Good"));

        let loaded = load_themes(&directory);

        assert_eq!(loaded.presets.len(), builtin_presets().len() + 1);
        assert!(loaded.warning.unwrap().contains("broken.json"));
    }

    #[test]
    fn bom_in_theme_file_is_tolerated() {
        let directory = test_directory("bom");
        let themes_directory = directory.join(THEMES_DIRECTORY);
        fs::create_dir_all(&themes_directory).unwrap();
        let json = format!(
            "\u{feff}{}",
            serde_json::to_string(&custom_preset("Bom")).unwrap()
        );
        fs::write(themes_directory.join("bom.json"), json).unwrap();

        let loaded = load_themes(&directory);

        assert_eq!(loaded.presets.last().unwrap().name, "Bom");
    }

    #[test]
    fn themes_path_that_is_a_file_is_reported() {
        let directory = test_directory("themes-as-file");
        fs::write(directory.join(THEMES_DIRECTORY), "not a folder").unwrap();

        let loaded = load_themes(&directory);

        assert_eq!(loaded.presets.len(), builtin_presets().len());
        assert!(loaded.warning.unwrap().contains("not a folder"));
    }

    #[test]
    fn migrates_legacy_file_into_folder_once() {
        let directory = test_directory("migration");
        let legacy = ThemeFile {
            themes: vec![builtin_presets()[1].clone(), custom_preset("Своя Dracula")],
        };
        fs::write(
            directory.join(LEGACY_THEME_FILE),
            serde_json::to_string_pretty(&legacy).unwrap(),
        )
        .unwrap();

        let loaded = load_themes(&directory);
        let themes_directory = directory.join(THEMES_DIRECTORY);
        let migrated_files = fs::read_dir(&themes_directory).unwrap().count();

        assert_eq!(migrated_files, 1);
        assert!(directory
            .join(LEGACY_THEME_FILE)
            .with_extension("json.old")
            .exists());
        assert!(find_preset_index(&loaded.presets, "Своя Dracula").is_some());
        assert!(loaded.warning.is_none());

        let again = load_themes(&directory);

        assert_eq!(again.presets.len(), loaded.presets.len());
        assert!(again.warning.is_none());
        assert_eq!(fs::read_dir(&themes_directory).unwrap().count(), 1);
    }

    #[test]
    fn migration_renames_edited_builtin_copy() {
        let directory = test_directory("migration-renamed");
        let mut edited = builtin_presets()[1].clone();
        edited.colors.accent = "#123456".to_string();
        fs::write(
            directory.join(LEGACY_THEME_FILE),
            serde_json::to_string_pretty(&ThemeFile {
                themes: vec![edited],
            })
            .unwrap(),
        )
        .unwrap();

        let loaded = load_themes(&directory);

        assert!(find_preset_index(&loaded.presets, "Dracula (custom)").is_some());
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn sanitizes_file_names() {
        assert_eq!(sanitize_theme_file_name("A/B: C?"), "A-B- C-");
        assert_eq!(sanitize_theme_file_name("  ..  "), "theme");
        assert_eq!(sanitize_theme_file_name("CON"), "_CON");
        assert_eq!(sanitize_theme_file_name(""), "theme");
    }

    #[test]
    fn applies_theme_without_restart() {
        let presets = builtin_presets();
        let dracula = find_preset_index(&presets, "Dracula").unwrap();

        apply_colors(&presets[dracula].colors);

        assert_eq!(theme().background, Color::Rgb(0x28, 0x2A, 0x36));
    }

    #[test]
    fn picks_readable_text_on_swatch() {
        assert_eq!(contrast_text(Color::Rgb(0xFF, 0xFF, 0xFF)), Color::Black);
        assert_eq!(contrast_text(Color::Rgb(0x08, 0x08, 0x08)), Color::White);
    }
}
