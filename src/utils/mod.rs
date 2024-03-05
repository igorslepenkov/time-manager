use chrono::Local;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::state::DailyState;

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

pub fn get_complited_tasks_names(state: &DailyState) -> Vec<String> {
    let complited_tasks_guard = state.completed_tasks.lock();
    if let Ok(tasks) = complited_tasks_guard {
        tasks
            .iter()
            .map(|task| task.name.to_owned())
            .to_owned()
            .collect::<Vec<String>>()
    } else {
        vec![]
    }
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
