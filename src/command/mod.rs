pub enum ManagerCommand {
    StartTrack,
    EndTrack,
    PauseTrack,
    Error,
}

pub fn get_command(command: &str) -> ManagerCommand {
    match command.trim() {
        "start" => ManagerCommand::StartTrack,
        "end" => ManagerCommand::EndTrack,
        "out" => ManagerCommand::PauseTrack,
        _ => ManagerCommand::Error,
    }
}
