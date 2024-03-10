use chrono::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct NotCompletedTask {
    pub name: String,
    pub dt_start: DateTime<Local>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CompletedTask {
    pub name: String,
    pub dt_start: DateTime<Local>,
    pub dt_end: DateTime<Local>,
    pub end_comment: Option<String>,
}

impl NotCompletedTask {
    pub fn start(name: String) -> NotCompletedTask {
        NotCompletedTask {
            name,
            dt_start: Local::now(),
        }
    }

    pub fn complete_task(&self, end_comment: Option<String>) -> CompletedTask {
        CompletedTask {
            name: self.name.to_string(),
            dt_start: self.dt_start.to_owned(),
            dt_end: Local::now(),
            end_comment,
        }
    }
}
