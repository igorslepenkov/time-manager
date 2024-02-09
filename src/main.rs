use colorful::Colorful;
use regex::Regex;
use std::{
    env,
    io::{self, BufRead},
    path::PathBuf,
};

use time_manager::{
    add_task_to_completed,
    command::{get_command, ManagerCommand},
    save_state_as_xlsx,
    state::{self, DailyState},
    task::NotCompletedTask,
};

const STATE_FILE_NAME: &str = "state.json";

fn main() {
    let current_exe_path = env::current_exe().unwrap();
    let current_dir_path = current_exe_path.parent().unwrap();
    let state_file_path = current_dir_path.join(STATE_FILE_NAME);

    let mut stdin = io::stdin().lock();

    let mut state: DailyState = state::DailyState::init(&state_file_path).unwrap();

    println!(
        "Started work at {}",
        state.start_time.format("%d/%m/%Y %H:%M")
    );

    loop {
        println!("Please enter your command");

        let mut command_string = String::new();
        let command_get_result = stdin.read_line(&mut command_string);

        match command_get_result {
            Ok(_) => {
                let parsed_arguments_result = get_command_arguments(command_string);

                match parsed_arguments_result {
                    Err(err) => {
                        println!("{}", err);

                        continue;
                    }
                    Ok(parsed_arguments) => {
                        let command_arg = parsed_arguments.get(0);

                        match command_arg {
                            Some(command_arg) => {
                                let command = get_command(command_arg);

                                let command_result = execute_command(
                                    &command,
                                    parsed_arguments,
                                    &mut state,
                                    &state_file_path,
                                );

                                match command_result {
                                    Err(err) => {
                                        let error_string = format!("{}", err);

                                        println!("{}", error_string.red());
                                    }
                                    Ok(result) => {
                                        let result_string = format!("{}", result);

                                        println!("{}", result_string.green());

                                        if let ManagerCommand::EndTrack = &command {
                                            break;
                                        }
                                    }
                                }
                            }
                            None => {
                                continue;
                            }
                        }
                    }
                }
            }
            Err(err) => {
                println!("{}", err);
                continue;
            }
        }
    }
}

fn get_command_arguments(arg_string: String) -> Result<Vec<String>, String> {
    let regex = Regex::new(r"^(\w+)\s*(?:'([^']+)')*\s*(?:'(.*)')*").unwrap();
    let arguments_match = regex.captures(&arg_string);

    match arguments_match {
        None => return Err("Could not parse arguments".to_string()),
        Some(capture) => {
            let mut arguments: Vec<String> = Vec::new();

            let command_string_option = capture.get(1);

            let command_string = match command_string_option {
                None => return Err("Could not get command".to_string()),
                Some(match_string) => match_string.as_str().to_string(),
            };

            arguments.insert(0, command_string);

            let task_name_option = capture.get(2);
            if let Some(task_name_match) = task_name_option {
                let string = task_name_match.as_str().to_string();
                arguments.insert(1, string);
            };

            let task_end_comment_option = capture.get(3);
            if let Some(task_end_comment_match) = task_end_comment_option {
                let string = task_end_comment_match.as_str().to_string();
                arguments.insert(2, string);
            }

            Ok(arguments)
        }
    }
}

fn execute_command(
    command: &ManagerCommand,
    args_vec: Vec<String>,
    state: &mut DailyState,
    state_file_path: &PathBuf,
) -> Result<String, String> {
    match command {
        ManagerCommand::StartTrack => execute_start_command(args_vec, state, state_file_path),

        ManagerCommand::PauseTrack => execute_pause_command(args_vec, state, state_file_path),

        ManagerCommand::EndTrack => execute_end_command(args_vec, state, state_file_path),

        ManagerCommand::Error => Err("Could not find command".to_string()),
    }
}

fn execute_start_command(
    args_vec: Vec<String>,
    state: &mut DailyState,
    state_file_path: &PathBuf,
) -> Result<String, String> {
    let task_name_option = args_vec.get(1);

    match task_name_option {
        None => Err("Please enter your task's name".to_string()),
        Some(task_name) => {
            let previous_task_completion_message = args_vec.get(2);
            let complete_task_result =
                complete_current_task(state, previous_task_completion_message);

            if let Err(err) = complete_task_result {
                return Err(err);
            }

            let new_task = NotCompletedTask::start(task_name.to_string());

            state.current_task.replace(new_task);

            let _ = state.save(state_file_path);

            Ok(format!("Started new task. Current task: {}", task_name))
        }
    }
}

fn execute_pause_command(
    args_vec: Vec<String>,
    state: &mut DailyState,
    state_file_path: &PathBuf,
) -> Result<String, String> {
    let previous_task_completion_message = args_vec.get(1);
    let complete_task_result = complete_current_task(state, previous_task_completion_message);

    state.current_task = None;

    if let Err(err) = complete_task_result {
        return Err(err);
    }

    let _ = state.save(state_file_path);

    println!("Track paused at {}", chrono::Local::now());

    Ok("Track is paused. Out of keyboard".to_string())
}

fn execute_end_command(
    args_vec: Vec<String>,
    state: &mut DailyState,
    state_file_path: &PathBuf,
) -> Result<String, String> {
    let previous_task_completion_message = args_vec.get(1);
    let complete_task_result = complete_current_task(state, previous_task_completion_message);

    if let Err(err) = complete_task_result {
        return Err(err);
    }

    let save_result = save_state_as_xlsx(state);

    match save_result {
        Ok(file_path) => {
            let _ = state.clear_todays_state(state_file_path);

            Ok(format!("Work is ended. Generated log file {}", file_path))
        }
        Err(_) => Err("Could not save workbook".to_string()),
    }
}

fn complete_current_task(
    state: &DailyState,
    task_completion_message: Option<&String>,
) -> Result<(), String> {
    let current_task_option = &state.current_task;

    if let Some(current_task) = current_task_option {
        let completed_current_task = current_task.complete_task(None);

        let add_result =
            add_task_to_completed(state, completed_current_task, task_completion_message);

        if let Err(err_string) = add_result {
            return Err(err_string);
        }
    }

    Ok(())
}
