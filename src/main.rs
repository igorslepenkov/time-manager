use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};

use anyhow::Result;

use std::{env, io::stdout, path::PathBuf};

use time_manager::app::App;

const STATE_FILE_NAME: &str = "state.json";

fn main() -> Result<()> {
    initialize_panic_handler();

    let current_exe_path = env::current_exe().unwrap();
    let current_dir_path = current_exe_path.parent().unwrap();
    let state_file_path: PathBuf = current_dir_path.join(STATE_FILE_NAME);

    let app = App::init(&state_file_path);

    startup()?;

    let status = app.run();

    shutdown()?;

    status?;

    Ok(())
}

fn startup() -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen).unwrap();
    Ok(())
}

fn shutdown() -> Result<()> {
    stdout().execute(LeaveAlternateScreen).unwrap();
    disable_raw_mode().unwrap();
    Ok(())
}

pub fn initialize_panic_handler() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        crossterm::execute!(std::io::stderr(), crossterm::terminal::LeaveAlternateScreen).unwrap();
        crossterm::terminal::disable_raw_mode().unwrap();
        original_hook(panic_info);
    }));
}
