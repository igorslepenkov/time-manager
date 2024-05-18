use std::sync::MutexGuard;

use chrono::Local;
use itertools::Itertools;
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

pub fn set_task_name_from_previous_tasks(
    input_state: &mut Input,
    completed_tasks: MutexGuard<'_, Vec<CompletedTask>>,
) {
    let current_task_name = &input_state.input;

    let task_search_idx_result = completed_tasks
        .iter()
        .find_position(|task| task.name == current_task_name.to_owned());

    match task_search_idx_result {
        Some((idx, _)) => {
            if idx == 0_usize {
                input_state.input = String::new()
            } else {
                let new_task_idx = idx - 1;
                let new_task = &completed_tasks[new_task_idx];

                input_state.input = new_task.name.to_owned();
            }
        }
        None => {
            let last_completed_task = completed_tasks.last().unwrap();

            input_state.input = last_completed_task.name.to_owned();
        }
    }
}

// pub fn predict_task_tag(task_text: String) -> String {
//     let tag_prediction_module_str =
//         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/python/app.py"));

//     let result = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
//         let app: Py<PyAny> = PyModule::from_code_bound(py, &tag_prediction_module_str, "", "")?
//             .getattr("run")?
//             .into();

//         let args = PyTuple::new_bound(py, &[task_text]);
//         app.call1(py, args)
//     });

//     match result {
//         Err(_err) => "".to_string(),
//         Ok(res) => res.to_string(),
//     }
// }
