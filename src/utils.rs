use std::sync::MutexGuard;

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::{state::DailyState, task::CompletedTask, ui::Input};

const MILLISECONDS_IN_HOUR: f64 = 3600000_f64;

pub fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn calculate_total_working_hours(state: &DailyState) -> f64 {
    let complited_tasks = state.completed_tasks.lock().unwrap();
    let current_task = &state.current_task;

    let result_milliseconds = complited_tasks.iter().fold(0i64, |acc, task| {
        let start = task.dt_start;
        let end = task.dt_end;
        let diff = end - start;

        acc + diff.num_milliseconds()
    });

    let result_in_hours = result_milliseconds as f64 / MILLISECONDS_IN_HOUR;

    if let Some(task) = current_task {
        let time_now = Local::now();
        let task_start_date = task.dt_start;

        let diff = time_now - task_start_date;
        let diff_in_hours = diff.num_milliseconds() as f64 / MILLISECONDS_IN_HOUR;
        result_in_hours + diff_in_hours
    } else {
        result_in_hours
    }
}

/// Суммарное время только по завершённым задачам — без идущей сейчас.
pub fn completed_hours(state: &DailyState) -> f64 {
    let completed_tasks = state.completed_tasks.lock().unwrap();
    let total_milliseconds = completed_tasks
        .iter()
        .map(|task| (task.dt_end - task.dt_start).num_milliseconds())
        .sum::<i64>();

    total_milliseconds as f64 / MILLISECONDS_IN_HOUR
}

/// Время, которым стоит закрыть вчерашнюю задачу: последняя активность
/// приложения, но не раньше старта задачи и не позже конца того дня.
pub fn suggested_close_time(
    last_activity: Option<DateTime<Local>>,
    task_start: Option<DateTime<Local>>,
    day: NaiveDate,
) -> DateTime<Local> {
    let end_of_day = day
        .and_hms_opt(23, 59, 59)
        .and_then(|naive| Local.from_local_datetime(&naive).earliest())
        .unwrap_or_else(Local::now);

    let candidate = last_activity.unwrap_or(end_of_day).min(end_of_day);

    match task_start {
        Some(task_start) => candidate.max(task_start),
        None => candidate,
    }
}

/// Раскрывает %ПЕРЕМЕННАЯ% и начальный ~ в пути из настроек.
pub fn expand_path(value: &str) -> String {
    let source = if let Some(rest) = value.strip_prefix('~') {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();

        format!("{home}{rest}")
    } else {
        value.to_string()
    };

    let mut result = String::new();
    let mut chars = source.chars();

    while let Some(char) = chars.next() {
        if char != '%' {
            result.push(char);

            continue;
        }

        let mut name = String::new();
        let mut closed = false;

        for next in chars.by_ref() {
            if next == '%' {
                closed = true;

                break;
            }

            name.push(next);
        }

        if closed && !name.is_empty() {
            match std::env::var(&name) {
                Ok(value) => result.push_str(&value),
                Err(_) => {
                    result.push('%');
                    result.push_str(&name);
                    result.push('%');
                }
            }
        } else {
            result.push('%');
            result.push_str(&name);
        }
    }

    result
}
/// Открывает файл программой, которая ассоциирована с расширением.
pub fn open_in_default_app(path: &std::path::Path) -> Result<(), String> {
    let path_string = path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", &path_string])
        .spawn();

    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(&path_string).spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open")
        .arg(&path_string)
        .spawn();

    result
        .map(|_| ())
        .map_err(|error| format!("Could not open {path_string}: {error}"))
}

pub fn set_task_name_from_previous_tasks(
    input_state: &mut Input,
    completed_tasks: MutexGuard<'_, Vec<CompletedTask>>,
) {
    match find_completed_task_index(&completed_tasks, &input_state.input) {
        Some(0) => input_state.set_input(String::new()),
        Some(idx) => input_state.set_input(completed_tasks[idx - 1].name.to_owned()),
        None => {
            if let Some(last_completed_task) = completed_tasks.last() {
                input_state.set_input(last_completed_task.name.to_owned());
            }
        }
    }
}

/// Номер выполненной задачи для поля ввода: сначала ищем точное совпадение,
/// потом — задачу, название которой начинается с введённого текста (в истории
/// к названию приписан комментарий вида «задача. комментарий»).
pub fn find_completed_task_index(tasks: &[CompletedTask], name: &str) -> Option<usize> {
    if name.is_empty() {
        return None;
    }

    tasks
        .iter()
        .rposition(|task| task.name == name)
        .or_else(|| tasks.iter().rposition(|task| task.name.starts_with(name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;
    use std::sync::Mutex;

    fn completed_task(name: &str) -> CompletedTask {
        CompletedTask {
            name: name.to_string(),
            dt_start: Local::now(),
            dt_end: Local::now(),
            end_comment: None,
        }
    }

    #[test]
    fn finds_completed_task_by_name() {
        let tasks = vec![
            completed_task("first"),
            completed_task("second"),
            completed_task("third"),
        ];

        assert_eq!(find_completed_task_index(&tasks, "second"), Some(1));
        assert_eq!(find_completed_task_index(&tasks, "third"), Some(2));
    }

    #[test]
    fn returns_none_for_unknown_or_empty_name() {
        let tasks = vec![completed_task("first")];

        assert_eq!(find_completed_task_index(&tasks, "unknown"), None);
        assert_eq!(find_completed_task_index(&tasks, ""), None);
        assert_eq!(find_completed_task_index(&[], "first"), None);
    }

    #[test]
    fn steps_back_through_completed_tasks() {
        let mut input = Input::init();
        let tasks = Mutex::new(vec![
            completed_task("first"),
            completed_task("second"),
            completed_task("third"),
        ]);

        set_task_name_from_previous_tasks(&mut input, tasks.lock().unwrap());
        assert_eq!(input.input, "third");

        set_task_name_from_previous_tasks(&mut input, tasks.lock().unwrap());
        assert_eq!(input.input, "second");

        set_task_name_from_previous_tasks(&mut input, tasks.lock().unwrap());
        assert_eq!(input.input, "first");

        set_task_name_from_previous_tasks(&mut input, tasks.lock().unwrap());
        assert_eq!(input.input, "");
    }
}

#[cfg(test)]
mod close_time_tests {
    use super::*;
    use chrono::Timelike;

    fn at(day: NaiveDate, hour: u32, minute: u32) -> DateTime<Local> {
        Local
            .from_local_datetime(&day.and_hms_opt(hour, minute, 0).unwrap())
            .earliest()
            .unwrap()
    }

    #[test]
    fn without_activity_suggests_the_end_of_the_day() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let close_at = suggested_close_time(None, None, day);

        assert_eq!(close_at.date_naive(), day);
        assert_eq!(close_at.hour(), 23);
    }

    #[test]
    fn uses_last_activity_when_it_is_after_task_start() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let task_start = at(day, 9, 0);
        let activity = at(day, 18, 30);

        assert_eq!(
            suggested_close_time(Some(activity), Some(task_start), day),
            activity
        );
    }

    #[test]
    fn never_closes_earlier_than_task_started() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let task_start = at(day, 9, 0);
        let activity = at(day, 7, 15);

        assert_eq!(
            suggested_close_time(Some(activity), Some(task_start), day),
            task_start
        );
    }

    #[test]
    fn never_closes_after_the_end_of_the_day() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let task_start = at(day, 9, 0);
        let next_morning = at(day.succ_opt().unwrap(), 10, 0);

        let close_at = suggested_close_time(Some(next_morning), Some(task_start), day);

        assert_eq!(close_at.date_naive(), day);
        assert_eq!(close_at.hour(), 23);
    }
}
#[cfg(test)]
mod path_tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn expands_environment_variables() {
        let separator = std::path::MAIN_SEPARATOR;
        let temp = std::env::var("TEMP").unwrap();

        assert_eq!(
            expand_path(&format!("%TEMP%{separator}reports")),
            format!("{temp}{separator}reports")
        );
    }

    #[cfg(windows)]
    #[test]
    fn expands_leading_home_shorthand() {
        let separator = std::path::MAIN_SEPARATOR;
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap();

        assert_eq!(
            expand_path(&format!("~{separator}docs")),
            format!("{home}{separator}docs")
        );
    }

    #[test]
    fn keeps_unknown_variables_and_plain_paths() {
        assert_eq!(expand_path("%NOT_A_REAL_VAR_42%"), "%NOT_A_REAL_VAR_42%");
        assert_eq!(expand_path("plain"), "plain");
    }
}
#[cfg(test)]
mod history_tests {
    use super::*;
    use chrono::Local;
    use std::sync::Mutex;

    fn completed_task(name: &str) -> CompletedTask {
        CompletedTask {
            name: name.to_string(),
            dt_start: Local::now(),
            dt_end: Local::now(),
            end_comment: None,
        }
    }

    #[test]
    fn matches_task_whose_name_has_a_comment_suffix() {
        let tasks = vec![
            completed_task("write report. done"),
            completed_task("read docs"),
        ];

        assert_eq!(find_completed_task_index(&tasks, "write report"), Some(0));
    }

    #[test]
    fn prefers_exact_match_over_prefix() {
        let tasks = vec![completed_task("write"), completed_task("write report")];

        assert_eq!(find_completed_task_index(&tasks, "write"), Some(0));
    }

    #[test]
    fn steps_back_from_task_name_typed_without_comment() {
        let mut input = Input::init();
        let tasks = Mutex::new(vec![
            completed_task("first"),
            completed_task("second. сделано"),
        ]);

        input.set_input("second".to_string());
        set_task_name_from_previous_tasks(&mut input, tasks.lock().unwrap());

        assert_eq!(input.input, "first");
    }
}
