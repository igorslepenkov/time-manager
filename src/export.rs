//! Сборка xlsx-книги за день и её сохранение на диск.

use std::collections::HashMap;
use std::env;
use std::fs::create_dir_all;
use std::path::PathBuf;

use chrono::prelude::*;
use rust_xlsxwriter::{
    ExcelDateTime, Format, FormatAlign, FormatBorder, Formula, Workbook, XlsxError,
};

use crate::config::{normalize_hex_color, ColumnKind, Config, ExportConfig};
use crate::i18n::t;
use crate::task::CompletedTask;

/// Куда будет сохранён файл дня — считается заранее, без записи на диск.
pub fn planned_file_path(config: &Config, start_time: DateTime<Local>) -> Result<String, String> {
    let output_directory = export_directory(&config.export.output_dir)?;
    let file_name = start_time.format(&config.export.sheet_name).to_string();
    let path = output_directory.join(format!("{}.xlsx", sanitize_file_name(&file_name)));

    Ok(path.to_string_lossy().to_string())
}

/// Собирает книгу по настройкам и сохраняет её. Возвращает путь к файлу.
pub fn write_workbook(
    config: &Config,
    start_time: DateTime<Local>,
    tasks: &[CompletedTask],
    task_tags: &[String],
) -> Result<String, String> {
    let mut workbook =
        build_workbook(config, start_time, tasks, task_tags).map_err(|error| error.to_string())?;

    let file_path = PathBuf::from(planned_file_path(config, start_time)?);
    let output_directory = file_path
        .parent()
        .map(|path| path.to_path_buf())
        .ok_or_else(|| {
            t(
                "Could not resolve the export folder",
                "Не удалось определить папку выгрузки",
            )
            .to_string()
        })?;

    create_dir_all(&output_directory).map_err(|error| {
        format!(
            "{} {}: {error}",
            t(
                "Could not create the export folder",
                "Не удалось создать папку выгрузки"
            ),
            output_directory.display()
        )
    })?;

    workbook
        .save(&file_path)
        .map_err(|error| error.to_string())?;

    if config.export.open_after_export {
        let _ = crate::utils::open_in_default_app(&file_path);
    }

    Ok(file_path.to_string_lossy().to_string())
}
fn build_workbook(
    config: &Config,
    start_time: DateTime<Local>,
    tasks: &[CompletedTask],
    task_tags: &[String],
) -> Result<Workbook, XlsxError> {
    let export = &config.export;
    let default_columns = ExportConfig::default().columns;
    let columns = if export.columns.is_empty() {
        &default_columns
    } else {
        &export.columns
    };

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    let sheet_name = start_time.format(&export.sheet_name).to_string();
    worksheet.set_name(&sanitize_sheet_name(&sheet_name))?;

    for (column_idx, column) in columns.iter().enumerate() {
        let column_idx = column_idx as u16;
        worksheet.set_column_width(column_idx, column.width())?;
    }

    let cell_formats = columns
        .iter()
        .map(|column| build_cell_format(export, column))
        .collect::<Vec<Format>>();

    if export.header_enabled {
        let header_format = build_header_format(export);

        for (column_idx, column) in columns.iter().enumerate() {
            worksheet.write_string_with_format(
                0,
                column_idx as u16,
                column.title(),
                &header_format,
            )?;
        }

        worksheet.set_row_height(0, export.row_height)?;
    }

    let first_data_row: u32 = if export.header_enabled { 1 } else { 0 };
    let section_letters = column_letters(columns);
    let task_seconds = tasks
        .iter()
        .map(|task| (task.dt_end - task.dt_start).num_seconds().max(0))
        .collect::<Vec<i64>>();

    for (task_idx, task) in tasks.iter().enumerate() {
        let row = first_data_row + task_idx as u32;
        worksheet.set_row_height(row, export.row_height)?;

        for (column_idx, column) in columns.iter().enumerate() {
            let column_number = column_idx as u16;
            let format = &cell_formats[column_idx];
            let tag = task_tags.get(task_idx).map(String::as_str).unwrap_or("");

            match column.kind {
                ColumnKind::Date => {
                    worksheet.write_date_with_format(
                        row,
                        column_number,
                        task.dt_start.date_naive(),
                        format,
                    )?;
                }
                ColumnKind::Tag => {
                    worksheet.write_string_with_format(row, column_number, tag, format)?;
                }
                ColumnKind::Task => {
                    worksheet.write_string_with_format(row, column_number, &task.name, format)?;
                }
                ColumnKind::Start => {
                    worksheet.write_time_with_format(
                        row,
                        column_number,
                        excel_time(&task.dt_start)?,
                        format,
                    )?;
                }
                ColumnKind::End => {
                    worksheet.write_time_with_format(
                        row,
                        column_number,
                        excel_time(&task.dt_end)?,
                        format,
                    )?;
                }
                ColumnKind::Duration | ColumnKind::Hours => {
                    if let Some(formula) = build_formula(column, row + 1, &section_letters) {
                        // Кэшированный результат, чтобы значение было видно даже
                        // в просмотрщиках, которые не пересчитывают формулы.
                        let cached = cached_result(column.kind, task_seconds[task_idx]);

                        worksheet.write_formula_with_format(
                            row,
                            column_number,
                            Formula::new(formula).set_result(cached),
                            format,
                        )?;
                    }
                }
                ColumnKind::Empty => {}
            }
        }
    }

    if export.totals_enabled && !tasks.is_empty() {
        if let Some(plan) = plan_totals(columns) {
            let totals_row = first_data_row + tasks.len() as u32;
            let first_excel_row = first_data_row + 1;
            let last_excel_row = first_data_row + tasks.len() as u32;
            let totals_sum_format =
                build_totals_format(export, &columns[plan.sum_column as usize].num_format());

            // Оформляем всю строку итога, а не только две ячейки с данными.
            worksheet.set_row_format(totals_row, &build_totals_format(export, ""))?;

            worksheet.write_string_with_format(
                totals_row,
                plan.label_column,
                export.totals_label.clone(),
                &build_totals_format(export, ""),
            )?;
            let totals_seconds: i64 = task_seconds.iter().sum();
            let totals_cached =
                cached_result(columns[plan.sum_column as usize].kind, totals_seconds);

            worksheet.write_formula_with_format(
                totals_row,
                plan.sum_column,
                Formula::new(totals_formula(
                    plan.sum_column,
                    first_excel_row,
                    last_excel_row,
                ))
                .set_result(totals_cached),
                &totals_sum_format,
            )?;
            worksheet.set_row_height(totals_row, export.row_height)?;
        }
    }

    Ok(workbook)
}

/// Куда писать итоговую строку: подпись — в колонку с задачей, сумма — в
/// колонку часов (или длительности, если часов нет).
struct TotalsPlan {
    label_column: u16,
    sum_column: u16,
}

fn plan_totals(columns: &[crate::config::ColumnConfig]) -> Option<TotalsPlan> {
    if columns.is_empty() {
        return None;
    }

    let label_column = columns
        .iter()
        .position(|column| column.kind == ColumnKind::Task)
        .or_else(|| {
            columns
                .iter()
                .position(|column| column.kind != ColumnKind::Empty)
        })?;

    let sum_column = columns
        .iter()
        .position(|column| column.kind == ColumnKind::Hours)
        .or_else(|| {
            columns
                .iter()
                .position(|column| column.kind == ColumnKind::Duration)
        })?;

    Some(TotalsPlan {
        label_column: label_column as u16,
        sum_column: sum_column as u16,
    })
}

fn totals_formula(column: u16, first_row: u32, last_row: u32) -> String {
    let letter = column_letter(column as u32);

    format!("=SUM({letter}{first_row}:{letter}{last_row})")
}

fn build_totals_format(export: &ExportConfig, num_format: &str) -> Format {
    let mut format = Format::new()
        .set_font_name(&export.font_name)
        .set_font_size(export.font_size)
        .set_font_color(normalize_hex_color(&export.text_color, "#000000").as_str())
        .set_background_color(
            normalize_hex_color(&export.header_background_color, "#DDDDDD").as_str(),
        )
        .set_bold()
        .set_border_top(FormatBorder::Thin);

    if !num_format.is_empty() {
        format = format.set_num_format(num_format);
    }

    format
}

/// Кэшированный результат формулы: часы — с двумя знаками, длительность —
/// в долях суток, как хранит Excel.
fn cached_result(kind: ColumnKind, seconds: i64) -> String {
    match kind {
        ColumnKind::Hours => format!("{:.2}", seconds as f64 / 3600.0),
        _ => format!("{:.10}", seconds as f64 / 86_400.0),
    }
}
/// Каталог для выгрузки: пустая настройка — рядом с исполняемым файлом.
fn export_directory(configured: &str) -> Result<PathBuf, String> {
    let trimmed = configured.trim();

    if trimmed.is_empty() {
        let executable = env::current_exe().map_err(|error| error.to_string())?;

        return executable
            .parent()
            .map(|path| path.to_path_buf())
            .ok_or_else(|| {
                t(
                    "Could not resolve executable directory",
                    "Не удалось определить папку программы",
                )
                .to_string()
            });
    }

    Ok(PathBuf::from(crate::utils::expand_path(trimmed)))
}

fn build_cell_format(export: &ExportConfig, column: &crate::config::ColumnConfig) -> Format {
    let mut format = Format::new()
        .set_font_name(&export.font_name)
        .set_font_size(export.font_size)
        .set_font_color(normalize_hex_color(&export.text_color, "#000000").as_str())
        .set_background_color(normalize_hex_color(&export.background_color, "#FFFFFF").as_str());

    if column.wrap() {
        format = format.set_text_wrap();
    }

    if column.bold() {
        format = format.set_bold();
    }

    let num_format = column.num_format();
    if !num_format.is_empty() {
        format = format.set_num_format(num_format);
    }

    format
}

fn build_header_format(export: &ExportConfig) -> Format {
    let mut format = Format::new()
        .set_font_name(&export.font_name)
        .set_font_size(export.font_size)
        .set_font_color(normalize_hex_color(&export.text_color, "#000000").as_str())
        .set_background_color(
            normalize_hex_color(&export.header_background_color, "#DDDDDD").as_str(),
        )
        .set_align(FormatAlign::VerticalCenter);

    if export.header_bold {
        format = format.set_bold();
    }

    format
}

fn excel_time(time: &DateTime<Local>) -> Result<ExcelDateTime, XlsxError> {
    let hour = time.hour() as u16;
    let minute = time.minute() as u8;
    let second = time.second() as f64;

    ExcelDateTime::from_hms(hour, minute, second)
}

/// Формула из настроек: подставляет номер строки и буквы колонок.
/// Если нужной колонки в таблице нет — формула не пишется.
fn build_formula(
    column: &crate::config::ColumnConfig,
    excel_row: u32,
    letters: &HashMap<ColumnKind, String>,
) -> Option<String> {
    let template = column.formula.as_ref()?;
    let mut formula = template.replace("{row}", &excel_row.to_string());

    let placeholders = [
        ("{start}", ColumnKind::Start),
        ("{end}", ColumnKind::End),
        ("{duration}", ColumnKind::Duration),
    ];

    for (placeholder, kind) in placeholders {
        if formula.contains(placeholder) {
            let letter = letters.get(&kind)?;
            formula = formula.replace(placeholder, letter);
        }
    }

    Some(formula)
}

fn column_letters(columns: &[crate::config::ColumnConfig]) -> HashMap<ColumnKind, String> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| (column.kind, column_letter(index as u32)))
        .collect()
}

fn column_letter(mut index: u32) -> String {
    let mut letters = String::new();

    loop {
        letters.insert(0, char::from(b'A' + (index % 26) as u8));

        if index < 26 {
            break;
        }

        index = index / 26 - 1;
    }

    letters
}

fn sanitize_sheet_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|char| match char {
            '[' | ']' | ':' | '*' | '?' | '/' | '\\' => '-',
            _ => char,
        })
        .take(31)
        .collect();

    if sanitized.trim().is_empty() {
        "Sheet".to_string()
    } else {
        sanitized
    }
}

fn sanitize_file_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|char| match char {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            _ => char,
        })
        .collect();

    if sanitized.trim().is_empty() {
        "export".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ColumnConfig;

    fn completed_task(name: &str, start: DateTime<Local>, minutes: i64) -> CompletedTask {
        CompletedTask {
            name: name.to_string(),
            dt_start: start,
            dt_end: start + chrono::Duration::minutes(minutes),
            end_comment: None,
        }
    }

    fn temp_directory() -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output");
        let _ = std::fs::create_dir_all(&directory);
        directory
    }

    #[test]
    fn converts_column_indexes_to_letters() {
        assert_eq!(column_letter(0), "A");
        assert_eq!(column_letter(4), "E");
        assert_eq!(column_letter(25), "Z");
        assert_eq!(column_letter(26), "AA");
        assert_eq!(column_letter(27), "AB");
    }

    #[test]
    fn builds_default_export() {
        let config = Config::default();
        let start = Local::now();
        let tasks = vec![
            completed_task("Write report", start, 45),
            completed_task("Read docs", start + chrono::Duration::minutes(45), 15),
        ];
        let tags = vec!["Docs".to_string(), "Docs".to_string()];

        let mut workbook = build_workbook(&config, start, &tasks, &tags).unwrap();
        let path = temp_directory().join("default_export.xlsx");
        workbook.save(&path).unwrap();

        assert!(path.exists());
        assert!(std::fs::metadata(&path).unwrap().len() > 0);
    }

    #[test]
    fn builds_export_with_header_and_custom_order() {
        let config: Config = serde_json::from_str(
            r##"{
                "export": {
                    "header_enabled": true,
                    "font_name": "Arial",
                    "columns": [
                        {"kind": "task", "width": 30},
                        {"kind": "start"},
                        {"kind": "end"},
                        {"kind": "duration", "formula": "={end}{row}-{start}{row}"},
                        {"kind": "hours"}
                    ]
                }
            }"##,
        )
        .unwrap();

        let start = Local::now();
        let tasks = vec![completed_task("Task", start, 30)];

        let mut workbook = build_workbook(&config, start, &tasks, &["T".to_string()]).unwrap();
        let path = temp_directory().join("header_export.xlsx");
        workbook.save(&path).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn skips_formula_when_column_is_missing() {
        let letters = HashMap::new();
        let column = ColumnConfig {
            kind: ColumnKind::Hours,
            formula: Some("={duration}{row}".to_string()),
            ..Default::default()
        };

        assert_eq!(build_formula(&column, 2, &letters), None);
    }

    #[test]
    fn substitutes_formula_placeholders() {
        let mut letters = HashMap::new();
        letters.insert(ColumnKind::Start, "D".to_string());
        letters.insert(ColumnKind::End, "E".to_string());

        let column = ColumnConfig {
            kind: ColumnKind::Duration,
            formula: Some("={end}{row}-{start}{row}".to_string()),
            ..Default::default()
        };

        assert_eq!(
            build_formula(&column, 3, &letters).unwrap(),
            "=E3-D3".to_string()
        );
    }

    #[test]
    fn writes_workbook_into_configured_directory() {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output")
            .join("export-configured");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        let mut config = Config::default();
        config.export.output_dir = directory.to_string_lossy().to_string();

        let start = Local::now()
            .date_naive()
            .and_hms_opt(9, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .earliest()
            .unwrap();
        let tasks = vec![CompletedTask {
            name: "work".to_string(),
            dt_start: start,
            dt_end: start + chrono::Duration::minutes(30),
            end_comment: None,
        }];

        let path = write_workbook(&config, start, &tasks, &[String::new()]).unwrap();
        let written = PathBuf::from(&path);

        assert_eq!(written.parent().unwrap(), directory.as_path());
        assert!(written.exists());

        let _ = std::fs::remove_file(&written);
    }

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_sheet_name("13.11.2026"), "13.11.2026");
        assert_eq!(sanitize_sheet_name("a/b:c"), "a-b-c");
        assert_eq!(sanitize_sheet_name(""), "Sheet");
        assert_eq!(sanitize_file_name("13.11.2026"), "13.11.2026");
        assert_eq!(sanitize_file_name("a/b"), "a-b");
    }
}

#[cfg(test)]
mod totals_tests {
    use super::*;
    use crate::config::ColumnConfig;

    fn column(kind: ColumnKind) -> ColumnConfig {
        ColumnConfig {
            kind,
            ..Default::default()
        }
    }

    #[test]
    fn totals_go_to_task_and_hours_columns() {
        let columns = Config::default().export.columns;
        let plan = plan_totals(&columns).unwrap();

        assert_eq!(plan.label_column, 2);
        assert_eq!(plan.sum_column, 6);
    }

    #[test]
    fn totals_fall_back_to_duration_column() {
        let columns = vec![
            column(ColumnKind::Task),
            column(ColumnKind::Start),
            column(ColumnKind::End),
            column(ColumnKind::Duration),
        ];
        let plan = plan_totals(&columns).unwrap();

        assert_eq!(plan.label_column, 0);
        assert_eq!(plan.sum_column, 3);
    }

    #[test]
    fn no_totals_without_time_columns() {
        let columns = vec![column(ColumnKind::Task), column(ColumnKind::Start)];

        assert!(plan_totals(&columns).is_none());
    }

    #[test]
    fn caches_results_in_the_same_units_as_columns() {
        assert_eq!(cached_result(ColumnKind::Hours, 3600), "1.00");
        assert_eq!(cached_result(ColumnKind::Hours, 5400), "1.50");
        assert_eq!(cached_result(ColumnKind::Duration, 3600), "0.0416666667");
    }

    #[test]
    fn totals_formula_covers_only_data_rows() {
        assert_eq!(totals_formula(6, 2, 4), "=SUM(G2:G4)");
        assert_eq!(totals_formula(6, 1, 1), "=SUM(G1:G1)");
    }

    #[test]
    fn export_directory_defaults_to_executable_folder() {
        let directory = export_directory("").unwrap();
        let executable = env::current_exe().unwrap();

        assert_eq!(directory, executable.parent().unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn export_directory_expands_variables() {
        let separator = std::path::MAIN_SEPARATOR;
        let directory = export_directory(&format!("%TEMP%{separator}tm-export")).unwrap();
        let expected = PathBuf::from(std::env::var("TEMP").unwrap()).join("tm-export");

        assert_eq!(directory, expected);
    }
}
