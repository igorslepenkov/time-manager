use std::env;
use std::fs::{remove_file, File, OpenOptions};
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::{
    task::{CompletedTask, NotCompletedTask},
    // utils::predict_task_tag,
};

use chrono::prelude::*;
use ort::{inputs, Session, Value};
use rust_xlsxwriter::{ExcelDateTime, Format, Formula, Workbook};
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

    pub fn save(&self, file_path: &PathBuf) -> Result<String, String> {
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

    pub fn clear_todays_state(&mut self, file_path: &PathBuf) -> Result<String, String> {
        self.completed_tasks = Mutex::new(vec![]);
        self.current_task = None;
        self.start_time = Local::now();
        self.end_time = None;

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
        let result_with_opened_file = File::open(file_path);

        match result_with_opened_file {
            Ok(mut file) => {
                let mut string = String::new();
                let read_result = file.read_to_string(&mut string);

                match read_result {
                    Ok(_) => {
                        let result_with_state: serde_json::Result<DailyState> =
                            serde_json::from_str(&string);

                        match result_with_state {
                            Ok(state) => Ok(state),
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
                let result_with_new_file = File::create(file_path);

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

    pub fn save_state_as_xlsx(&mut self, model: &Session) -> Result<String, ()> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        let date_format = Format::new()
            .set_num_format("hh:mm")
            .set_text_wrap()
            .set_background_color("#EEEEEE")
            .set_font_name("Nunito")
            .set_font_size(10)
            .set_bold();

        let date = self.start_time.format("%d.%m.%Y").to_string();

        let _ = worksheet.set_name(&date);

        let _ = worksheet.set_column_width(0, 15);
        let _ = worksheet.set_column_width(2, 40);

        let _ = worksheet.set_name(&date);

        let _ = worksheet.write_with_format(0, 0, &date, &date_format);

        let completed_tasks = self.completed_tasks.lock().unwrap();

        for (idx, task) in completed_tasks.to_vec().iter().enumerate() {
            let date_format = Format::new()
                .set_num_format("hh:mm")
                .set_text_wrap()
                .set_background_color("#EEEEEE")
                .set_font_name("Nunito")
                .set_font_size(10);

            let start_time_xlsx = ExcelDateTime::from_hms(
                task.dt_start.hour().try_into().unwrap(),
                task.dt_start.minute().try_into().unwrap(),
                task.dt_start.second(),
            )
            .unwrap();

            let end_time_xlsx = ExcelDateTime::from_hms(
                task.dt_end.hour().try_into().unwrap(),
                task.dt_end.minute().try_into().unwrap(),
                task.dt_end.second(),
            )
            .unwrap();

            let row_idx: u32 = idx.try_into().unwrap();

            let _ = worksheet.set_row_height(row_idx, 50);

            let time_difference_formula = Formula::new(format!("=E{0}-D{0}", row_idx + 1));
            let hours_total_formula = Formula::new(format!(
                "=ROUND(HOUR(F{0})+MINUTE(F{0})/60+SECOND(F{0})/3600, 2)",
                row_idx + 1
            ));

            let task_name_format = Format::new()
                .set_text_wrap()
                .set_background_color("#EEEEEE")
                .set_font_name("Nunito")
                .set_font_size(10);
            let time_difference_format = Format::new()
                .set_text_wrap()
                .set_background_color("#EEEEEE")
                .set_font_name("Nunito")
                .set_font_size(10);
            let hours_total_format = Format::new()
                .set_text_wrap()
                .set_background_color("#EEEEEE")
                .set_font_name("Nunito")
                .set_bold()
                .set_font_size(10);

            let x = vec![task.name.to_owned()];
            let allocator = model.allocator();
            let data = ([x.len()], x.into_boxed_slice());
            let input = inputs![Value::from_string_array(allocator, data)?].unwrap();

            let prediction_model_outputs = model.run(input).unwrap();

            let (_length, task_tags) = &prediction_model_outputs[0]
                .try_extract_raw_string_tensor()
                .unwrap();

            let _ = worksheet.write(row_idx, 1, &task_tags[0]);
            let _ =
                worksheet.write_with_format(row_idx, 2, task.name.to_owned(), &task_name_format);
            let _ = worksheet.write_with_format(row_idx, 3, start_time_xlsx, &date_format);
            let _ = worksheet.write_with_format(row_idx, 4, end_time_xlsx, &date_format);
            let _ = worksheet.write_formula_with_format(
                row_idx,
                5,
                time_difference_formula,
                &time_difference_format,
            );
            let _ = worksheet.write_formula_with_format(
                row_idx,
                6,
                hours_total_formula,
                &hours_total_format,
            );
        }

        let current_exe_path = env::current_exe().unwrap();
        let current_dir_path = current_exe_path.parent().unwrap();
        let current_dir_path_string = current_dir_path.to_str().unwrap();
        let file_path = format!("{}/{}.xlsx", current_dir_path_string, date);

        let save_result = workbook.save(&file_path);

        match save_result {
            Ok(_) => Ok(file_path.to_string()),
            Err(_) => Err(()),
        }
    }

    pub fn complete_current_task(
        &mut self,
        task_completion_message: Option<String>,
    ) -> Result<(), String> {
        let task_to_complite_option = self.current_task.take();

        if task_to_complite_option.is_none() {
            return Ok(());
        }

        let task_to_complite = task_to_complite_option.unwrap();
        let mut complited_task = task_to_complite.complete_task(task_completion_message);

        let tasks = self.completed_tasks.lock();

        match tasks {
            Ok(mut tasks) => {
                let mut new_tasks_state = tasks.to_vec();

                if let Some(message) = &complited_task.end_comment {
                    complited_task.name =
                        [complited_task.name.to_owned(), message.to_owned()].join(". ");
                }

                new_tasks_state.push(complited_task);

                *tasks = new_tasks_state;

                Ok(())
            }
            Err(err) => Err(err.to_string()),
        }
    }
}
