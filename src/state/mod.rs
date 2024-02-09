use std::fs::{remove_file, File, OpenOptions};
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::task::{CompletedTask, NotCompletedTask};

use chrono::prelude::*;
use colorful::Colorful;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct DailyState {
    pub current_task: Option<NotCompletedTask>,
    pub completed_tasks: Mutex<Vec<CompletedTask>>,
    pub start_time: DateTime<Local>,
    pub end_time: Option<DateTime<Local>>,
}

impl DailyState {
    pub fn init(file_path: &PathBuf) -> Result<DailyState, String> {
        DailyState::fetch_or_init_state(file_path)
    }

    pub fn save(self: &Self, file_path: &PathBuf) -> Result<String, String> {
        let result_with_string = serde_json::to_string(self);

        match result_with_string {
            Ok(string) => {
                let result_with_opened_file =
                    OpenOptions::new().write(true).read(true).open(file_path);

                let file = match result_with_opened_file {
                    Ok(file) => Ok(file),
                    Err(_) => {
                        let result_with_new_file = File::create(file_path);

                        match result_with_new_file {
                            Ok(new_file) => Ok(new_file),
                            Err(_) => Err(()),
                        }
                    }
                };

                match file {
                    Ok(mut file) => {
                        let buffer = string.as_bytes();
                        let write_result = file.write_all(buffer);

                        match write_result {
                            Ok(_) => Ok("State saved".to_owned()),
                            Err(_) => Err("Coudl not write state to file".to_owned()),
                        }
                    }
                    Err(_) => Err("Could not create or open state file".to_owned()),
                }
            }
            Err(_) => Err("Could not save state as JSON".to_owned()),
        }
    }

    pub fn clear_todays_state(self: &Self, file_path: &PathBuf) -> Result<String, String> {
        let result = remove_file(file_path);

        match result {
            Ok(_) => Ok("Today's state has been cleared".to_owned()),
            Err(_) => Err("Could not clear today's state".to_owned()),
        }
    }

    fn init_new_state_in_file(file_path: &PathBuf) -> Result<DailyState, ()> {
        let new_state = DailyState {
            current_task: None,
            completed_tasks: Default::default(),
            start_time: Local::now(),
            end_time: None,
        };

        let _ = new_state.save(file_path);

        Ok(new_state)
    }

    fn fetch_or_init_state(file_path: &PathBuf) -> Result<DailyState, String> {
        let result_with_opened_file = File::open(&file_path);

        match result_with_opened_file {
            Ok(mut file) => {
                let mut string = String::new();
                let read_result = file.read_to_string(&mut string);

                match read_result {
                    Ok(_) => {
                        let result_with_state: serde_json::Result<DailyState> =
                            serde_json::from_str(&string);

                        match result_with_state {
                            Ok(state) => {
                                if let Some(task) = &state.current_task {
                                    println!("{}", "Current task:".to_string().green());
                                    println!("{}", task.name.to_owned().green());
                                }

                                Ok(state)
                            }
                            Err(_) => {
                                let new_state =
                                    DailyState::init_new_state_in_file(file_path).unwrap();

                                Ok(new_state)
                            }
                        }
                    }
                    Err(_) => Err("Could not read state file".to_owned()),
                }
            }
            Err(_) => {
                let result_with_new_file = File::create(&file_path);

                match result_with_new_file {
                    Ok(_) => {
                        let new_state = DailyState::init_new_state_in_file(file_path).unwrap();

                        Ok(new_state)
                    }
                    Err(_) => Err("Initialization fault".to_owned()),
                }
            }
        }
    }
}
