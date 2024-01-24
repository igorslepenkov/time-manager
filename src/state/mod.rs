use std::sync::Mutex;

use crate::task::{CompletedTask, NotCompletedTask};

use chrono::prelude::*;

pub struct DailyState {
    pub current_task: Option<NotCompletedTask>,
    pub completed_tasks: Mutex<Vec<CompletedTask>>,
    pub start_time: DateTime<Local>,
    pub end_time: Option<DateTime<Local>>,
}

impl DailyState {
    pub fn init() -> DailyState {
        DailyState {
            current_task: None,
            completed_tasks: Default::default(),
            start_time: Local::now(),
            end_time: None,
        }
    }
}
