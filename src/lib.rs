pub mod command;
pub mod state;
pub mod task;
pub mod theme;
pub mod ui;
pub mod utils;

// pub fn save_state_as_xlsx(state: &mut DailyState) -> Result<String, ()> {
//     let mut workbook = Workbook::new();
//     let worksheet = workbook.add_worksheet();

//     let date_format = Format::new()
//         .set_num_format("hh:mm")
//         .set_text_wrap()
//         .set_background_color("#EEEEEE")
//         .set_font_name("Nunito")
//         .set_font_size(10)
//         .set_bold();

//     let date = state.start_time.format("%d.%m.%Y").to_string();

//     let _ = worksheet.set_name(&date);

//     let _ = worksheet.write_with_format(0, 0, &date, &date_format);

//     let completed_tasks = state.completed_tasks.lock().unwrap();

//     for (idx, task) in completed_tasks.to_vec().iter().enumerate() {
//         let date_format = Format::new()
//             .set_num_format("hh:mm")
//             .set_text_wrap()
//             .set_background_color("#EEEEEE")
//             .set_font_name("Nunito")
//             .set_font_size(10);

//         let start_time_xlsx = ExcelDateTime::from_hms(
//             task.dt_start.hour().try_into().unwrap(),
//             task.dt_start.minute().try_into().unwrap(),
//             task.dt_start.second(),
//         )
//         .unwrap();

//         let end_time_xlsx = ExcelDateTime::from_hms(
//             task.dt_end.hour().try_into().unwrap(),
//             task.dt_end.minute().try_into().unwrap(),
//             task.dt_end.second(),
//         )
//         .unwrap();

//         let row_idx: u32 = idx.try_into().unwrap();

//         let time_difference_formula = Formula::new(format!("=E{0}-D{0}", row_idx + 1));
//         let hours_total_formula = Formula::new(format!(
//             "=ROUND(HOUR(F{0})+MINUTE(F{0})/60+SECOND(F{0})/3600, 2)",
//             row_idx + 1
//         ));

//         let task_name_format = Format::new()
//             .set_text_wrap()
//             .set_background_color("#EEEEEE")
//             .set_font_name("Nunito")
//             .set_font_size(10);
//         let time_difference_format = Format::new()
//             .set_text_wrap()
//             .set_background_color("#EEEEEE")
//             .set_font_name("Nunito")
//             .set_font_size(10);
//         let hours_total_format = Format::new()
//             .set_text_wrap()
//             .set_background_color("#EEEEEE")
//             .set_font_name("Nunito")
//             .set_bold()
//             .set_font_size(10);

//         let _ = worksheet.write_with_format(row_idx, 2, task.name.to_owned(), &task_name_format);
//         let _ = worksheet.write_with_format(row_idx, 3, start_time_xlsx, &date_format);
//         let _ = worksheet.write_with_format(row_idx, 4, end_time_xlsx, &date_format);
//         let _ = worksheet.write_formula_with_format(
//             row_idx,
//             5,
//             time_difference_formula,
//             &time_difference_format,
//         );
//         let _ = worksheet.write_formula_with_format(
//             row_idx,
//             6,
//             hours_total_formula,
//             &hours_total_format,
//         );
//     }

//     let current_exe_path = env::current_exe().unwrap();
//     let current_dir_path = current_exe_path.parent().unwrap();
//     let current_dir_path_string = current_dir_path.to_str().unwrap();
//     let file_path = format!("{}/{}.xlsx", current_dir_path_string, date);

//     let save_result = workbook.save(&file_path);

//     match save_result {
//         Ok(_) => Ok(file_path.to_string()),
//         Err(_) => Err(()),
//     }
// }

// pub fn add_task_to_completed(
//     state: &state::DailyState,
//     mut task: CompletedTask,
//     task_completion_message: Option<&String>,
// ) -> Result<(), String> {
//     let tasks = state.completed_tasks.lock();

//     match tasks {
//         Ok(mut tasks) => {
//             let mut new_tasks_state = tasks.to_vec();

//             if let Some(message) = task_completion_message {
//                 task.name = [task.name, message.to_string()].join(". ");
//             }

//             new_tasks_state.push(task);

//             *tasks = new_tasks_state;

//             Ok(())
//         }
//         Err(err) => Err(err.to_string()),
//     }
// }
