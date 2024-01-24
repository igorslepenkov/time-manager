use task::CompletedTask;

pub mod command;
pub mod state;
pub mod task;

pub fn add_task_to_completed(
    state: &state::DailyState,
    mut task: CompletedTask,
    task_completion_message: Option<&String>,
) -> Result<(), String> {
    let tasks = state.completed_tasks.lock();

    match tasks {
        Ok(mut tasks) => {
            let mut new_tasks_state = tasks.to_vec();

            if let Some(message) = task_completion_message {
                task.name = [task.name, message.to_string()].join(". ");
            }

            new_tasks_state.push(task);

            *tasks = new_tasks_state;

            Ok(())
        }
        Err(err) => Err(err.to_string()),
    }
}
